extern crate ffmpeg_next as ffmpeg;

use ffmpeg::codec::Id as CodecId;
use ffmpeg::codec::decoder::Video as VideoDecoder;
use ffmpeg::format::{Pixel, context::Input, input};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::frame::video::Video;
use std::mem;
use std::ops::Range;
use std::path::Path;

use crate::responces::Response;
use crate::util::result::EsotereelResult;
use anyhow::Context;

const AV_TIME_BASE: f64 = 1_000_000.0;
pub struct VideoStreamer {
    pub ictx: Input,
    decoder: VideoDecoder,
    scaler: Scaler,
    pub video_stream_index: usize,
    pub time_base: f64,
    pub last_pts: Option<i64>,
    needs_discontinuity_flag: bool,
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
            last_pts: None,
            needs_discontinuity_flag: false,
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
            self.last_pts = decoded.pts();
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

    pub fn get_init_packet(&self, path: &str, resource_id: u32) -> Response {
        let codec_id = self.codec_id();
        let width = self.width();
        let height = self.height();
        let extradata = self.extradata().unwrap_or(&[]).to_vec();
        let time_base = self.time_base;

        let codec_id = unsafe { std::mem::transmute(codec_id) };

        Response::StreamMetadata {
            path: path.to_owned(),
            resource_id,
            codec_id,
            width,
            height,
            time_base,
            extradata,
        }
    }

    pub fn fetch_stream_data(
        &mut self,
        resource_id: u32,
        seek_sec_range: Range<f64>,
    ) -> EsotereelResult<Vec<Response>> {
        log::info!(
            "fetch_stream_data called: seek_seconds={:?}",
            seek_sec_range
        );

        let current_time = self.last_pts.map(|pts| pts as f64 * self.time_base);
        let needs_seek = match current_time {
            Some(t) => {
                // 現在位置が要求開始位置より後にある場合、または
                // 前方に離れすぎている場合はシークを実行する
                t > seek_sec_range.start || t < seek_sec_range.start - 1.0
            }
            None => true,
        };

        if needs_seek {
            self.seek(seek_sec_range.start)
                .context("Failed to seek during range request")?;
            self.needs_discontinuity_flag = true;
        }

        let mut to_sends = Vec::new();

        // packets()ではなくread_packet()を直接ループ
        let mut packet = ffmpeg::Packet::empty();
        while packet.read(&mut self.ictx).is_ok() {
            if packet.stream() != self.video_stream_index {
                continue;
            }

            let pts = packet.pts();
            let dts = packet.dts();
            let packet_time = pts.map(|p| p as f64 * self.time_base).unwrap_or(0.0);

            self.last_pts = pts;

            let res = Response::StreamData {
                resource_id,
                data: packet.data().map(|d| d.to_vec()).unwrap_or_default(),
                pts,
                dts,
                is_key: packet.is_key(),
                discontinuous: mem::take(&mut self.needs_discontinuity_flag),
            };
            to_sends.push(res);

            if packet_time >= seek_sec_range.end {
                break;
            }
        }

        // 完了通知
        to_sends.push(Response::StreamDataEnd {
            resource_id,
            fetched_range: seek_sec_range,
        });

        Ok(to_sends)
    }

    /// 指定した秒数（timestamp）にシークする
    pub fn seek(&mut self, seconds: f64) -> Result<(), ffmpeg::Error> {
        let timestamp = (seconds * AV_TIME_BASE) as i64;
        self.ictx.seek(timestamp, ..timestamp)?;
        self.decoder.flush();
        self.last_pts = None;
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
