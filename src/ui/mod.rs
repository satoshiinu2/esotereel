use egui_wgpu::wgpu;

use crate::{project::Project, ui::wgpuutil::WGpuUtil};

pub(super) mod preview;
pub(super) mod timeline;
pub(crate) mod wgpuutil;

pub struct WindowState {
    pub wgpuutil: WGpuUtil,
    pub behavior: Box<dyn WindowBehavior>,
    pub visible: bool,
    pub need_redraw: bool,
}

impl WindowState {
    pub fn new(wgpuutil: WGpuUtil, behaivior: Box<dyn WindowBehavior>) -> Self {
        Self {
            wgpuutil,
            behavior: behaivior,
            visible: true,
            need_redraw: true,
        }
    }

    pub(crate) fn can_render(&self) -> bool {
        let size = self.wgpuutil.window.inner_size();
        self.need_redraw && self.visible && size.width != 0 && size.height != 0
    }
}

#[allow(unused_variables)]
pub trait WindowBehavior {
    fn title(&self) -> String;
    fn size(&self) -> [f32; 2];

    fn update(&mut self, project: &mut Option<Project>, ctx: &egui::Context);

    fn init_special_renderer(&mut self, wgpuutil: &WGpuUtil) {}

    fn render_special<'a>(
        &mut self,
        project: &mut Option<Project>,
        rpass: &mut wgpu::RenderPass<'a>,
        wgpuutil: &WGpuUtil,
    ) {
    }
}
