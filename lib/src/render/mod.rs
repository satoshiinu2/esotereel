use wgpu::CurrentSurfaceTexture;

use crate::render::{vertex::Vertex, wgpuutil::WGpuUtil};

pub mod init;
pub mod pipeline;
pub mod surfacetarget;
pub mod uniform;
pub mod vertex;
pub mod wgpuutil;

pub fn render_frame(util: &mut WGpuUtil) {
    if util.config.width == 0 || util.config.height == 0 {
        return;
    }

    // 2. 次のフレーム用のテクスチャを取得
    let output = match util.surface.get_current_texture() {
        CurrentSurfaceTexture::Success(t) => t,
        _ => return,
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = util
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    // --- ここにあなたのロジックを組み込む ---
    // ※ プロジェクトデータ(project)をどこから持ってくるかは、別途ポインタで渡すか
    // WGpuUtilの中やグローバルに保持しておく必要があります
    let mut vertices = vec![];

    // 仮のplayheadとデータ構造でのループ例
    // 本来は project ポインタなどから取得してください
    let playhead = 0.0;
    let layer_height = 80.0;

    // (あなたのロジックで vertices を作成...)
    vertices.extend_from_slice(&Vertex::rect(
        100.0,
        100.0,
        200.0,
        layer_height,
        [0.2, 0.5, 0.8, 1.0],
    ));

    let screen_size = [util.config.width as f32, util.config.height as f32];

    {
        // 3. RenderPass の作成
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

        // 4. 描画実行（pipeline.rs の render から write_buffer を除いたもの）
        if !vertices.is_empty() {
            util.resources
                .render(&util.queue, &mut rpass, screen_size, &vertices[..]);
        }
    }

    // 5. GPUへ送信
    util.queue.submit(std::iter::once(encoder.finish()));
    output.present();
}
