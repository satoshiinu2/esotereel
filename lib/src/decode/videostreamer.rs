extern crate ffmpeg_next as ffmpeg;

use ffmpeg::codec::Id as CodecId;
use ffmpeg::codec::decoder::Video as VideoDecoder;
use ffmpeg::format::{Pixel, context::Input, input};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::frame::video::Video;
use std::path::Path;

pub struct VideoStreamer {
    pub ictx: Input,
    decoder: VideoDecoder,
    scaler: Scaler,
    pub video_stream_index: usize,
    pub time_base: f64,
}

impl VideoStreamer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, ffmpeg::Error> {
        let ictx = input(&path)?;

        let stream = ictx
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;

        let video_stream_index = stream.index();
        let time_base = f64::from(stream.time_base());

        let context_decoder =
            ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        let decoder = context_decoder.decoder().video()?;

        // 出力サイズを指定できるようにすると、プレビュー時に低解像度でデコードできて高速
        let scaler = Scaler::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            Pixel::RGBA,
            decoder.width(),
            decoder.height(),
            Flags::BILINEAR,
        )?;

        Ok(Self {
            ictx,
            decoder,
            scaler,
            video_stream_index,
            time_base,
        })
    }

    /// 次のフレームをデコードして取得する
    pub fn next_frame(&mut self) -> Option<Video> {
        let mut decoded = Video::empty();

        // パケットを読み込み、デコーダに流し込む
        for (stream, packet) in self.ictx.packets() {
            if stream.index() != self.video_stream_index {
                continue;
            }

            // デコーダへの送信または受信が失敗した場合は次のパケットへ
            if self.decoder.send_packet(&packet).is_err() {
                continue;
            }
            if self.decoder.receive_frame(&mut decoded).is_err() {
                continue;
            }

            // スケーリング処理
            let mut rgb_frame =
                Video::new(Pixel::RGBA, self.decoder.width(), self.decoder.height());
            self.scaler.run(&decoded, &mut rgb_frame).ok()?;
            rgb_frame.set_pts(decoded.pts()); // PTSをコピーして保持
            return Some(rgb_frame);
        }
        None
    }

    /// 指定した秒数のフレームをピンポイントで取得する
    pub fn get_frame_at_time(&mut self, seconds: f64) -> Option<Video> {
        // 1. 指定位置の直前のキーフレームへシーク
        if self.seek(seconds).is_err() {
            return None;
        }

        // 2. 目的の秒数を超えるまでデコードを進める
        while let Some(frame) = self.next_frame() {
            let current_pts = frame.pts().unwrap_or(0) as f64 * self.time_base;
            if current_pts >= seconds {
                return Some(frame);
            }
        }
        None
    }

    /// 指定した秒数（timestamp）にシークする
    pub fn seek(&mut self, seconds: f64) -> Result<(), ffmpeg::Error> {
        let timestamp = (seconds * f64::from(ffmpeg::util::mathematics::rescale::TIME_BASE)) as i64;
        self.ictx.seek(timestamp, ..timestamp)?; // 近くのIフレームに飛ぶ
        self.decoder.flush(); // デコーダ内部のバッファをクリア
        Ok(())
    }

    pub fn codec_id(&self) -> CodecId {
        self.decoder.id()
    }

    pub fn width(&self) -> u32 {
        self.decoder.width()
    }

    pub fn height(&self) -> u32 {
        self.decoder.height()
    }

    /// extradataが存在しない場合を考慮し、Optionで包む
    pub fn extradata(&self) -> Option<&[u8]> {
        unsafe {
            let codec_context = self.decoder.as_ptr();
            let data = (*codec_context).extradata;
            if data.is_null() {
                None
            } else {
                let size = (*codec_context).extradata_size;
                Some(std::slice::from_raw_parts(data, size as usize))
            }
        }
    }
}
