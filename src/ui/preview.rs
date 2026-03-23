use egui_wgpu::wgpu;

use crate::{
    project::Project,
    render::{pipeline::WgpuRenderResources, vertex::Vertex},
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
        project: &mut Option<Project>,
        rpass: &mut wgpu::RenderPass<'_>,
        wgpuutil: &WGpuUtil,
    ) {
        let Some(project) = &project else {
            return;
        };
        let Some(render_res) = &self.render_res else {
            return;
        };

        let timeline = &project.timeline;
        let playhead = timeline.playhead;

        let mut vertices = vec![];
        for (layer_idx, layer) in timeline.layers.iter().enumerate() {
            for clip in &layer.clips {
                if playhead >= clip.position && playhead < clip.position + clip.duration {
                    let y = layer_idx as f32 * 100.0; // レイヤーごとにY座標をずらす（仮）
                    vertices.extend_from_slice(&Vertex::rect(
                        100.0,
                        y,
                        200.0,
                        80.0,
                        [0.2, 0.5, 0.8, 1.0],
                    ));
                }
            }
        }
        let width = wgpuutil.config.width;
        let height = wgpuutil.config.height;

        render_res.render(
            &wgpuutil.queue,
            rpass,
            [width as f32, height as f32],
            &vertices,
        );
    }

    fn init_special_renderer(&mut self, wgpuutil: &WGpuUtil) {
        self.render_res = Some(WgpuRenderResources::new(
            &wgpuutil.device,
            wgpuutil.config.format,
        ));
    }

    fn update(&mut self, _project: &mut Option<Project>, _ctx: &egui::Context) {}
}
