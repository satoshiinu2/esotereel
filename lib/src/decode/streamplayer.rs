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

const BUFFER_KEEP_SECONDS_BEFORE: f64 = 2.0; // 現在位置より前に保持する秒数
const BUFFER_KEEP_SECONDS_AFTER: f64 = 5.0; // 現在位置より後に保持する秒数

pub enum FetchState {
    Idle,
    Fetching {
        requested_at: time::Instant,
        seek_range_sec: Range<f64>,
    },
}

impl FetchState {
    const FETCH_TIMEOUT: time::Duration = time::Duration::from_secs(10);

    pub fn is_active(&self) -> bool {
        match self {
            FetchState::Idle => false,
            FetchState::Fetching { requested_at, .. } => {
                requested_at.elapsed() < Self::FETCH_TIMEOUT
            }
        }
    }
}

pub struct StreamPlayer {
    decoder: VideoDecoder,
    scaler: Option<Scaler>,
    target_width: u32,
    target_height: u32,
    frame_count: u32,
    pub frames: BTreeMap<ordered_float::OrderedFloat<f64>, Video>, // (秒数, フレーム) のペアで保持
    pub time_base: f64,                                            // 1単位あたりの秒数 (1/fps 等)
    pub fetch_state: FetchState,
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
    /// メタデータからデコーダとスケーラを初期化
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
        })
    }

    /// サーバーから届いたパケット（バイナリ）をデコードしてフレームを返す
    pub fn process_packet(
        &mut self,
        packet_data: &[u8],
        pts: Option<i64>,
        dts: Option<i64>,
        is_key: bool,
        discontinuous: bool,
    ) -> Result<(), ffmpeg::util::error::Error> {
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
    pub fn free_no_needed_frames(&mut self, fetched_range: Range<f64>) {
        let keep_start = OrderedFloat(fetched_range.start - BUFFER_KEEP_SECONDS_BEFORE);
        let keep_end = OrderedFloat(fetched_range.end + BUFFER_KEEP_SECONDS_AFTER);

        self.frames = self.frames.split_off(&keep_start);
        self.frames.split_off(&keep_end);
    }

    /// 指定した秒数に最も近いフレームをバッファから取得する
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

    pub fn flush(&mut self) {
        // self.fetch_state = FetchState::Idle;
        self.decoder.flush();
        // self.frames.clear(); // Clear buffered frames on flush
    }

    /// デバッグ用: PPM形式で保存する内部メソッド
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
