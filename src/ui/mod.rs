use std::{
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

use crate::{
    project::{Project, clip::ClipDragState},
    ui::{
        event::{SubWindowEvent, SubWindowEventQueue},
        wgpuutil::WGpuUtil,
    },
};

pub mod event;
pub mod preview;
pub mod timeline;
mod timelinedrag;
mod timelinescroll;
pub mod wgpuutil;

pub struct WindowState {
    pub wgpuutil: WGpuUtil,
    pub behavior: Rc<Mutex<dyn WindowBehavior>>,
    pub visible: bool,
}

impl WindowState {
    pub fn new(wgpuutil: WGpuUtil, behaivior: Rc<Mutex<dyn WindowBehavior>>) -> Self {
        Self {
            wgpuutil,
            behavior: behaivior,
            visible: true,
        }
    }
}

pub trait WindowBehavior {
    fn id(&self) -> egui::ViewportId;
    fn title(&self) -> String;
    fn size(&self) -> [f32; 2];

    fn update(
        &mut self,
        project: Arc<RwLock<Option<Project>>>,
        drag_state: Arc<RwLock<Option<ClipDragState>>>,
        ctx: &egui::Context,
    );
}
