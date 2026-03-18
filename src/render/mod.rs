use std::sync::{Arc, Mutex};

use egui_wgpu::wgpu;

use crate::render::resources::WgpuRenderResources;

pub mod callback;
pub mod resources;

pub struct RenderState {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
}

pub struct FrameBuffer {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
pub fn start_render_thread(render_state: Arc<RenderState>) {
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let resources = WgpuRenderResources::new(&render_state.device, format, 1920, 1080);

    std::thread::spawn(move || {
        loop {
            // wgpuでオフスクリーンレンダリング
            let pixels = resources.render_to_buffer(&render_state.device, &render_state.queue);

            // フレームバッファに書き込む
            *render_state.frame_buffer.lock().unwrap() = Some(FrameBuffer {
                data: pixels,
                width: 1920,
                height: 1080,
            });

            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    });
}
