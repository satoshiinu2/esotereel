extern crate ffmpeg_next as ffmpeg;

use ffmpeg::util::frame::video::Video;

use crate::{
    StreamState,
    project::{clip_data::ClipData, timeline::Timeline},
    render::wgpuutil::WGpuUtil,
};

pub mod request;

pub(crate) fn update_timline_clips_texture(
    util: &mut WGpuUtil,
    app_state: &crate::ClientState,
    timeline: &Timeline,
    current_frame: i64,
) {
    for layer in &timeline.layers {
        if let Some(clip) = layer.clips.get_at(current_frame) {
            if let ClipData::Video { path, media_offset } = &clip.clip_data {
                if let Some(resource_id_ref) = app_state.path_to_stream.get(path) {
                    if let StreamState::Loaded(resource_id) = *resource_id_ref {
                        if let Some(player) = app_state.streams.get(&resource_id) {
                            let media_seconds = ClipData::get_media_seconds(
                                timeline.fps,
                                clip.position(),
                                current_frame,
                                *media_offset,
                            );

                            if let Some(video_frame) = player.get_frame_at(media_seconds) {
                                ensure_and_update_texture(util, resource_id, video_frame);
                            }
                        }
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
