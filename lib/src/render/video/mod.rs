extern crate ffmpeg_next as ffmpeg;

use std::ops::Range;

use dashmap::DashMap;
use ffmpeg::util::frame::video::Video;

use crate::{
    StreamState,
    project::{clip::ClipData, ids::ClipId},
    render::{RenderContext, wgpuutil::WGpuUtil},
};

pub mod builder;
pub mod request;

// render/plugin.rs
#[derive(Default)]
pub struct MediaFetchCache {
    // clip_id -> (path, needed_seconds群)
    needs: DashMap<ClipId, Vec<(String, Range<f64>)>>,

    init_requests: DashMap<String, ()>, // Setとして使う(重複自動排除)
}

impl MediaFetchCache {
    pub fn record(&self, clip_id: ClipId, path: String, range: Range<f64>) {
        self.needs.entry(clip_id).or_default().push((path, range));
    }

    /// 前フレーム分を取り出してクリアする(このフレームのフェッチ計算に使う)
    pub fn take_all(&self) -> Vec<(String, Range<f64>)> {
        let mut all = Vec::new();
        for mut entry in self.needs.iter_mut() {
            all.append(entry.value_mut());
        }
        all
    }

    pub fn record_init_request(&self, path: String) {
        self.init_requests.insert(path, ());
    }

    pub fn take_init_requests(&self) -> Vec<String> {
        let keys: Vec<String> = self.init_requests.iter().map(|e| e.key().clone()).collect();
        self.init_requests.clear();
        keys
    }
}
pub(crate) fn update_timline_clips_texture(util: &mut WGpuUtil, ctx: &RenderContext) {
    for (path, range) in ctx.media_fetch_cache.take_all() {
        if let Some(resource_id_ref) = ctx.path_to_stream.get(&path) {
            if let StreamState::Loaded(resource_id) = *resource_id_ref {
                if let Some(player) = ctx.streams.get(&resource_id) {
                    if let Some(video_frame) = player.get_frame_at(range.start) {
                        ensure_and_update_texture(util, resource_id, video_frame);
                    }
                }
            }
        }
    }
}

fn ensure_and_update_texture(util: &mut WGpuUtil, resource_id: u32, frame: &Video) {
    let (width, height) = (frame.width(), frame.height());
    let mut should_recreate = false;

    if let Some((texture, _)) = util.textures.get(&resource_id) {
        if texture.width() != width || texture.height() != height {
            should_recreate = true;
        }
    } else {
        should_recreate = true;
    }

    if should_recreate {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = util.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("video_texture_{}", resource_id)),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: util.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });

        let sampler = util.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = util.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("video_bg_{}", resource_id)),
            layout: &util.resources.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        util.textures.insert(resource_id, (texture, bind_group));
    }

    // update texture
    if let Some((texture, _)) = util.textures.get(&resource_id) {
        util.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            frame.data(0),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.stride(0) as u32),
                rows_per_image: Some(height),
            },
            texture.size(),
        );
    }
}
