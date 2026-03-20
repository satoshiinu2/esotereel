use egui::Pos2;
use egui_wgpu::wgpu;
use winit::dpi::PhysicalPosition;

use crate::{
    project::{Project, clip::ClipDragState},
    ui::wgpuutil::WGpuUtil,
};

pub(super) mod preview;
pub(super) mod timeline;
mod timelinedrag;
mod timelinescroll;
pub(crate) mod wgpuutil;

pub struct WindowState {
    pub wgpuutil: WGpuUtil,
    pub behavior: Box<dyn WindowBehavior>,
    pub visible: bool,
    pub need_redraw: bool,
    pub cursor_pos: Option<PhysicalPosition<f64>>,
    pub(crate) need_reconfigure: bool,
}

impl WindowState {
    pub fn new(wgpuutil: WGpuUtil, behaivior: Box<dyn WindowBehavior>) -> Self {
        Self {
            wgpuutil,
            behavior: behaivior,
            visible: true,
            need_redraw: true,
            cursor_pos: None,
            need_reconfigure: false,
        }
    }
}

pub trait WindowBehavior {
    fn title(&self) -> String;
    fn size(&self) -> [f32; 2];

    fn update(&mut self, project: &mut Option<Project>, ctx: &egui::Context);

    #[allow(unused_variables)]
    fn init_special_renderer(&mut self, wgpuutil: &WGpuUtil) {}

    #[allow(unused_variables)]
    fn render_special(
        &mut self,
        project: &mut Option<Project>,
        rpass: &mut wgpu::RenderPass<'_>,
        wgpuutil: &WGpuUtil,
    ) {
    }

    #[allow(unused_variables)]
    fn on_drag_grab(
        &mut self,
        project: Option<&mut Project>,
        drag: &mut Option<ClipDragState>,
        local: Pos2,
    ) {
    }
    #[allow(unused_variables)]
    fn on_drag_continue(
        &mut self,
        project: Option<&mut Project>,
        drag: &mut Option<ClipDragState>,
        local: Pos2,
    ) {
    }

    #[allow(unused_variables)]
    fn on_drag_drop(
        &mut self,
        project: Option<&mut Project>,
        drag: &mut Option<ClipDragState>,
        local: Pos2,
    ) {
    }
}
