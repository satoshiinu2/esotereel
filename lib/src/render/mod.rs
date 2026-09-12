use crate::{
    StreamState,
    decode::streamplayer::StreamPlayer,
    project::{Timeline, TimelineTick, camera::CameraInfo, ids::ResourceId},
    render::{video::update_timline_clips_texture, wgpuutil::OffscreenTarget},
};
use dashmap::DashMap;
use glam::Mat4;

use crate::render::{vertex::Vertex, video::builder::build_vertices, wgpuutil::WGpuUtil};

pub mod pipeline;
pub mod uniform;
pub mod vertex;
pub mod video;
pub mod wgpuutil;

pub struct RenderContext<'a> {
    pub path_to_stream: &'a DashMap<String, StreamState>,
    pub streams: &'a DashMap<ResourceId, StreamPlayer>,

    pub timeline: &'a Timeline,
    pub camera_info: &'a CameraInfo,
    pub current_frame: TimelineTick,
}

pub struct RenderBatch {
    pub vertices: Vec<Vertex>,
    pub texture_bind_group: wgpu::BindGroup,
    pub transform: Mat4,
}

pub fn render_frame_offscreen(
    util: &mut WGpuUtil,
    offscreen: &OffscreenTarget,
    ctx: &RenderContext,
) -> Result<(), String> {
    if offscreen.width == 0 || offscreen.height == 0 {
        return Err("window size is 0".into());
    }

    let screen_size = [offscreen.width as f32, offscreen.height as f32];

    // ビュープロジェクション行列の計算
    let proj_matrix = ctx.camera_info.get_proj_mat(screen_size);
    let view_matrix = ctx.camera_info.get_view_mat();

    let view_projection_matrix = proj_matrix * view_matrix;

    // 頂点作成
    let vertices = build_vertices(ctx);

    let batches: Vec<RenderBatch> = if !vertices.is_empty() {
        vertices
            .into_iter()
            .map(|b| {
                let bind_group = util
                    .textures
                    .get(&b.texture_id)
                    .map(|(_, bg)| bg.clone())
                    .unwrap_or_else(|| util.resources.dummy_bind_group.clone());

                RenderBatch {
                    vertices: b.vertices,
                    texture_bind_group: bind_group,
                    transform: b.transform,
                }
            })
            .collect()
    } else {
        vec![]
    };

    // バッファの更新（RenderPass開始前に行う）
    if !batches.is_empty() {
        util.resources.update_buffers(
            &util.device,
            &util.queue,
            screen_size,
            view_projection_matrix,
            &batches,
        );
    }

    let view = &offscreen.view;
    let mut encoder = util
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Encoder"),
        });

    // 各レイヤーのビデオフレームを確認し、GPUテクスチャを更新する
    update_timline_clips_texture(util, ctx);

    {
        // RenderPass の作成
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // 背景色（少し暗いグレー）
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        // 描画コマンドの記録
        if !batches.is_empty() {
            util.resources
                .record_render_commands(&mut rpass, &batches, &util.device);
        }
    }

    // 5. GPUへ送信
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &offscreen.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &offscreen.buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(offscreen.padded_bytes_per_row),
                rows_per_image: Some(offscreen.height),
            },
        },
        wgpu::Extent3d {
            width: offscreen.width,
            height: offscreen.height,
            depth_or_array_layers: 1,
        },
    );

    util.queue.submit(std::iter::once(encoder.finish()));
    Ok(())
}
