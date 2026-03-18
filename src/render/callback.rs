use egui_wgpu::wgpu;

use crate::render::resources::WgpuRenderResources;

pub struct WgpuRenderCallback;

impl egui_wgpu::CallbackTrait for WgpuRenderCallback {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let res = callback_resources.get::<WgpuRenderResources>().unwrap();
        render_pass.set_pipeline(&res.pipeline);
        render_pass.draw(0..3, 0..1);
    }
}
