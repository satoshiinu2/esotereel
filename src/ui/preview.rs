use egui_wgpu::wgpu;

use crate::{
    project::{Project, clip::ClipDragState},
    render::WgpuRenderResources,
    ui::{WindowBehavior, wgpuutil::WGpuUtil},
};

pub struct PreviewWindow {
    render_res: Option<WgpuRenderResources>,
}

impl Default for PreviewWindow {
    fn default() -> Self {
        Self { render_res: None }
    }
}

impl WindowBehavior for PreviewWindow {
    fn title(&self) -> String {
        "Preview".to_string()
    }

    fn size(&self) -> [f32; 2] {
        [800.0, 300.0]
    }
    fn render_special(
        &mut self,
        _project: &mut Option<Project>,
        rpass: &mut wgpu::RenderPass<'_>,
        _wgpuutil: &WGpuUtil,
    ) {
        let Some(render_res) = &self.render_res else {
            return;
        };

        rpass.set_pipeline(&render_res.pipeline);
        // rpass.set_vertex_buffer(0, render_res.vertex_buffer.slice(..));

        rpass.draw(0..3, 0..1);
    }

    fn init_special_renderer(&mut self, wgpuutil: &WGpuUtil) {
        self.render_res = Some(WgpuRenderResources::new(
            &wgpuutil.device,
            wgpuutil.config.format,
        ));
    }

    fn update(&mut self, _project: &mut Option<Project>, _ctx: &egui::Context) {}
}
