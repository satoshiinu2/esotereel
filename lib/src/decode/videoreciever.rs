extern crate ffmpeg_next as ffmpeg;

use ffmpeg::codec::decoder::Video as VideoDecoder;
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::frame::video::Video;
use std::fmt;
use std::io::Write;

pub struct StreamReciever {
    decoder: VideoDecoder,
    scaler: Option<Scaler>,
    target_width: u32,
    target_height: u32,
    pub last_frame: Option<Video>,
    frame_count: u32,
}

impl fmt::Debug for StreamReciever {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemotePlayer")
            .field("decoder", &self.decoder.id())
            .field(
                "target_resolution",
                &(self.target_width, self.target_height),
            )
            .finish()
    }
}

impl StreamReciever {
    /// メタデータからデコーダとスケーラを初期化
    pub fn new_from_metadata(
        codec_id: ffmpeg_next::codec::Id,
        width: u32,
        height: u32,
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
            last_frame: None,
            frame_count: 0,
        })
    }

    /// サーバーから届いたパケット（バイナリ）をデコードしてフレームを返す
    pub fn process_packet(
        &mut self,
        packet_data: &[u8],
        pts: Option<i64>,
        dts: Option<i64>,
        is_key: bool,
    ) -> Option<&Video> {
        let mut packet = ffmpeg::codec::packet::Packet::copy(packet_data);
        packet.set_pts(pts);
        packet.set_dts(dts);
        if is_key {
            packet.set_flags(ffmpeg::codec::packet::Flags::KEY);
        }

        // デコーダへの送信処理
        self.decoder.send_packet(&packet).ok()?;

        let mut decoded = Video::empty();
        self.decoder.receive_frame(&mut decoded).ok()?;

        // 初回フレーム受信時にスケーラを遅延初期化
        if self.scaler.is_none() {
            self.scaler = Scaler::get(
                decoded.format(),
                decoded.width(),
                decoded.height(),
                Pixel::RGB24,
                self.target_width,
                self.target_height,
                Flags::BILINEAR,
            )
            .ok();
        }

        let mut rgb_frame = Video::empty();
        self.scaler.as_mut()?.run(&decoded, &mut rgb_frame).ok()?;

        // デバッグ処理の分離
        if self.frame_count <= 5 {
            self.save_debug_frame(&rgb_frame);
        }

        self.last_frame = Some(rgb_frame);
        self.last_frame.as_ref()
    }

    pub fn flush(&mut self) {
        self.decoder.flush();
    }

    /// デバッグ用: PPM形式で保存する内部メソッド
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
                let end = start + width * 3;
                let _ = file.write_all(&data[start..end]);
            }
        }
        self.frame_count += 1;
    }
}
