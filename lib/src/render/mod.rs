use crate::{project::camera::CameraInfo, render::video::update_timline_clips_texture};
use glam::Mat4;
use wgpu::CurrentSurfaceTexture;

use crate::{
    ClientState,
    project::timeline::Timeline,
    render::{builder::build_vertices, vertex::Vertex, wgpuutil::WGpuUtil},
};

pub mod builder;
pub mod pipeline;
pub mod surfacetarget;
pub mod uniform;
pub mod vertex;
pub mod video;
pub mod wgpuutil;

pub struct RenderBatch {
    pub vertices: Vec<Vertex>,
    pub texture_bind_group: wgpu::BindGroup,
    pub transform: Mat4,
}

pub fn render_frame(
    util: &mut WGpuUtil,
    timeline: &Timeline,
    app_state: &ClientState,
    camera_info: &CameraInfo,
    current_frame: i64,
) -> Result<(), String> {
    if util.config.width == 0 || util.config.height == 0 {
        return Err("window size is 0".into());
    }

    let output = match util.surface.get_current_texture() {
        CurrentSurfaceTexture::Success(t) => t,
        CurrentSurfaceTexture::Suboptimal(t) => t,
        _ => return Err("could not get next frame texture".into()),
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = util
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Encoder"),
        });

    // 各レイヤーのビデオフレームを確認し、GPUテクスチャを更新する
    update_timline_clips_texture(util, app_state, timeline, current_frame);

    let screen_size = [util.config.width as f32, util.config.height as f32];

    // ビュープロジェクション行列の計算
    let proj_matrix = camera_info.get_proj_mat(screen_size);
    let view_matrix = camera_info.get_view_mat();

    let view_projection_matrix = proj_matrix * view_matrix;

    // 頂点作成
    let vertices = build_vertices(timeline, app_state, current_frame);

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
    util.queue.submit(std::iter::once(encoder.finish()));
    output.present();
    Ok(())
}
