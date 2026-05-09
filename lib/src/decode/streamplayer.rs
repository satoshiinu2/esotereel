extern crate ffmpeg_next as ffmpeg;

use ffmpeg::codec::decoder::Video as VideoDecoder;
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::frame::video::Video;
use std::collections::VecDeque;
use std::fmt;
use std::io::Write;

pub struct StreamPlayer {
    decoder: VideoDecoder,
    scaler: Option<Scaler>,
    target_width: u32,
    target_height: u32,
    frame_count: u32,
    pub frames: VecDeque<(f64, Video)>, // (秒数, フレーム) のペアで保持
    pub time_base: f64,                 // 1単位あたりの秒数 (1/fps 等)
    pub last_requested_time: Option<f64>,
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
            frames: VecDeque::new(),
            time_base,
            last_requested_time: None,
        })
    }

    /// サーバーから届いたパケット（バイナリ）をデコードしてフレームを返す
    pub fn process_packet(
        &mut self,
        packet_data: &[u8],
        pts: Option<i64>,
        dts: Option<i64>,
        is_key: bool,
    ) -> Result<(), ffmpeg::util::error::Error> {
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
                    // if self.frame_count <= 5 {
                    //     self.save_debug_frame(&rgb_frame);
                    // }

                    // PTSを秒数に変換して保持
                    let timestamp = decoded.pts().unwrap_or(0) as f64 * self.time_base;

                    if self.frames.len() >= 600 {
                        // 最大保持数
                        self.frames.pop_front();
                    }
                    self.frames.push_back((timestamp, rgb_frame));

                    log::debug!(
                        "Decoded frame at timestamp: {:.3}s (buffer size: {})",
                        timestamp,
                        self.frames.len()
                    )
                }
            }
        }

        Ok(())
    }

    /// 指定した秒数に最も近いフレームをバッファから取得する
    pub fn get_frame_at(&self, seconds: f64) -> Option<&Video> {
        let (time, frame) = self.frames.iter().min_by(|(a_time, _), (b_time, _)| {
            (a_time - seconds)
                .abs()
                .partial_cmp(&(b_time - seconds).abs())
                .unwrap()
        })?;

        // 指定時間から0.1秒以上離れている場合は「見つからない」と判定（要調整）
        if (time - seconds).abs() > 0.1 {
            return None;
        }
        Some(frame)
    }

    pub fn flush(&mut self) {
        self.decoder.flush();
        self.frames.clear(); // Clear buffered frames on flush
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
