extern crate ffmpeg_next as ffmpeg;

use ffmpeg::codec::decoder::Video as VideoDecoder;
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::frame::video::Video;
use ordered_float::OrderedFloat;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::ops::Range;
use std::time;

use crate::project::MediaSec;

/// 現在位置より前に保持する秒数（バッファの前方マージン）。
const BUFFER_KEEP_SECONDS_BEFORE: MediaSec = 2.0;
/// 現在位置より後に保持する秒数（バッファの後方マージン）。
const BUFFER_KEEP_SECONDS_AFTER: MediaSec = 5.0;

/// ストリームのフェッチ状態を表します。
///
/// `Idle` はアイドル、`Fetching` はリクエスト時刻と求める範囲を保持した
/// フェッチ中状態です。`FETCH_TIMEOUT`（10秒）を超えると非アクティブとみなします。
pub enum FetchState {
    Idle,
    Fetching {
        requested_at: time::Instant,
        seek_ranges: Vec<Range<f64>>,
    },
}

impl FetchState {
    /// フェッチがタイムアウトとみなされる時間（10秒）。
    const FETCH_TIMEOUT: time::Duration = time::Duration::from_secs(10);

    /// フェッチがアクティブ（タイムアウト前）かを返します。
    pub fn is_active(&self) -> bool {
        match self {
            FetchState::Idle => false,
            FetchState::Fetching { requested_at, .. } => {
                requested_at.elapsed() < Self::FETCH_TIMEOUT
            }
        }
    }
}

/// ストリームプレイヤーです。
///
/// デコーダ・スケーラを持ち、サーバーから届いたパケットをデコードして
/// フレームバッファ（`frames`）に保持します。`active_windows` に基づき
/// 不要なフレームを `free_no_needed_frames` で解放し、メモリ使用量を抑えます。
///
/// `frames` は `OrderedFloat<MediaSec>`（秒数）をキーに `Video` フレームを保持し、
/// `get_frame_at` で指定秒数に最も近いフレームを取得できます。
pub struct StreamPlayer {
    decoder: VideoDecoder,
    scaler: Option<Scaler>,
    target_width: u32,
    target_height: u32,
    frame_count: u32,
    current_generation: u64,

    /// 秒数 → フレーム のマップ。バッファされたデコード済みフレームを保持。
    pub frames: BTreeMap<ordered_float::OrderedFloat<MediaSec>, Video>,
    /// 1単位あたりの秒数（1/fps 等）。
    pub time_base: MediaSec,
    pub fetch_state: FetchState,
    /// 現在アクティブな再生ウィンドウの範囲一覧。
    pub active_windows: Vec<Range<MediaSec>>,
}

impl fmt::Debug for StreamPlayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamReciever")
            .field("decoder", &self.decoder.id())
            .field(
                "target_resolution",
                &(self.target_width, self.target_height),
            )
            .finish()
    }
}

impl StreamPlayer {
    /// メタデータからデコーダとスケーラを初期化します。
    ///
    /// `unsafe` で FFmpeg の `AVCodecContext` に幅・高さ・extradata を直接設定し、
    /// extradata の末尾には FFmpeg が要求するパディングを付加します。
    pub fn new_from_metadata(
        codec_id: ffmpeg::codec::Id,
        width: u32,
        height: u32,
        time_base: f64,
        extradata: &[u8],
    ) -> Result<Self, ffmpeg::Error> {
        let codec = ffmpeg::decoder::find(codec_id).ok_or(ffmpeg::Error::DecoderNotFound)?;
        let mut context = ffmpeg::codec::context::Context::new_with_codec(codec);

        // unsafeを使用して内部のAVCodecContextに値をセットする
        unsafe {
            let codec_context = context.as_mut_ptr();
            (*codec_context).width = width as i32;
            (*codec_context).height = height as i32;

            if !extradata.is_empty() {
                let size = extradata.len();
                // FFmpegはextradataの末尾にパディングを要求する
                let alloc_size = size + ffmpeg::sys::AV_INPUT_BUFFER_PADDING_SIZE as usize;
                let data = ffmpeg::sys::av_malloc(alloc_size);

                if !data.is_null() {
                    std::ptr::copy_nonoverlapping(extradata.as_ptr(), data as *mut u8, size);
                    std::ptr::write_bytes(
                        (data as *mut u8).add(size),
                        0,
                        ffmpeg::sys::AV_INPUT_BUFFER_PADDING_SIZE as usize,
                    );
                    (*codec_context).extradata = data as *mut u8;
                    (*codec_context).extradata_size = size as i32;
                }
            }
        }

        let decoder = context.decoder().video()?;

        Ok(Self {
            decoder,
            scaler: None,
            target_width: width,
            target_height: height,
            frame_count: 0,
            frames: BTreeMap::new(),
            time_base,
            fetch_state: FetchState::Idle,
            current_generation: 0,
            active_windows: Vec::new(),
        })
    }

    /// メタデータで指定されたデコード画像の解像度を返します。
    ///
    /// 現在は元動画と同じサイズです。
    pub fn dimensions(&self) -> (u32, u32) {
        (self.target_width, self.target_height)
    }

    /// サーバーから届いたパケット（バイナリ）をデコードしてフレームバッファに保持します。
    ///
    /// 古い世代のパケットは無視し、`discontinuous` フラグが立っている場合はデコーダをフラッシュします。
    /// 初回フレーム受信時にスケーラを遅延初期化し、RGBA フレームを `frames` に挿入します。
    pub fn process_packet(
        &mut self,
        packet_data: &[u8],
        pts: Option<i64>,
        dts: Option<i64>,
        is_key: bool,
        discontinuous: bool,
        generation: u64,
    ) -> Result<(), ffmpeg::util::error::Error> {
        if generation < self.current_generation {
            return Ok(()); // 古い世代のパケットは無視(重なりで来た過去のフェッチの残骸)
        }
        self.current_generation = generation;

        if discontinuous {
            self.flush();
        }

        let mut packet = ffmpeg::codec::packet::Packet::copy(packet_data);
        packet.set_pts(pts);
        packet.set_dts(dts);
        if is_key {
            packet.set_flags(ffmpeg::codec::packet::Flags::KEY);
        }

        self.decoder.send_packet(&packet)?;

        let mut decoded = Video::empty();

        // 利用可能なフレームをすべて取り出すループ
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            // 初回フレーム受信時にスケーラを遅延初期化
            if self.scaler.is_none() {
                self.scaler = Scaler::get(
                    decoded.format(),
                    decoded.width(),
                    decoded.height(),
                    Pixel::RGBA,
                    self.target_width,
                    self.target_height,
                    Flags::BILINEAR,
                )
                .ok();
            }

            if let Some(scaler) = self.scaler.as_mut() {
                // 出力バッファを明示的に確保 (RGBA 4bytes/pixel)
                let mut rgb_frame = Video::new(Pixel::RGBA, self.target_width, self.target_height);
                if scaler.run(&decoded, &mut rgb_frame).is_ok() {
                    // PTSを秒数に変換して保持
                    let timestamp = decoded.pts().unwrap_or(0) as f64 * self.time_base;

                    self.frames.insert(OrderedFloat(timestamp), rgb_frame);
                }
            }
        }

        Ok(())
    }

    /// 不要なフレームをバッファから解放します。
    ///
    /// `active_ranges` の各範囲（前後 `BUFFER_KEEP_SECONDS` 分をマージン）に含まれない
    /// フレームを削除し、メモリ使用量を抑えます。
    pub fn free_no_needed_frames(&mut self, active_ranges: &[Range<f64>]) {
        if active_ranges.is_empty() {
            return;
        }

        self.frames.retain(|OrderedFloat(t), _| {
            active_ranges.iter().any(|r| {
                let keep_start = r.start - BUFFER_KEEP_SECONDS_BEFORE;
                let keep_end = r.end + BUFFER_KEEP_SECONDS_AFTER;
                *t >= keep_start && *t <= keep_end
            })
        });
    }

    /// 指定秒数に最も近いフレームをバッファから取得します。
    ///
    /// 前後のフレームを比較し、差分が 0.1 秒以内の場合のみ `Some` を返します。
    /// それ以外は `None` を返します。
    pub fn get_frame_at(&self, seconds: f64) -> Option<&Video> {
        let target = OrderedFloat(seconds);

        let before = self.frames.range(..=target).next_back();
        let after = self.frames.range(target..).next();

        let binding = [before, after];
        let (sec, frame) = binding
            .iter()
            .flatten()
            .min_by_key(|(p, _)| OrderedFloat((**p - seconds).abs()))?;

        if (**sec - seconds).abs() > 0.1 {
            return None;
        }
        Some(frame)
    }

    /// デコーダをフラッシュし、バッファされた内部状態をリセットします。
    pub fn flush(&mut self) {
        // self.fetch_state = FetchState::Idle;
        self.decoder.flush();
        // self.frames.clear(); // Clear buffered frames on flush
    }

    /// デバッグ用：フレームを PPM 形式で保存する内部メソッドです。
    #[allow(dead_code)]
    fn save_debug_frame(&mut self, frame: &Video) {
        let path = format!("debug_frame_{}.ppm", self.frame_count);
        if let Ok(mut file) = std::fs::File::create(&path) {
            let header = format!("P6\n{} {}\n255\n", frame.width(), frame.height());
            let _ = file.write_all(header.as_bytes());

            let width = frame.width() as usize;
            let stride = frame.stride(0);
            let data = frame.data(0);

            for y in 0..frame.height() as usize {
                let start = y * stride;
                for x in 0..width {
                    let px = start + x * 4;
                    let _ = file.write_all(&data[px..px + 3]);
                }
            }
        }
        self.frame_count += 1;
    }
}
