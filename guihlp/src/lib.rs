use std::sync::{OnceLock, RwLock};

pub use esotereel_lib::project::Project;
pub use esotereel_lib::project::clip::Clip;
pub use esotereel_lib::project::layer::Layer;
pub use esotereel_lib::project::timeline::Timeline;
use esotereel_lib::responces::set_responce_callbacks;

use crate::responces::on_responce_recveve;

pub mod project;
pub mod responces;
pub mod wrapper;

pub(crate) static PROJECT: RwLock<Option<Project>> = RwLock::new(None);
static GUI_CALLBACKS: OnceLock<GuiCallbacks> = OnceLock::new();

#[repr(C)]
pub enum WrapperErrorCode {
    Ok = 0,
    NullPtr = 1,
    NotFound = 2,
    Panic = 3,
}
#[repr(C)]
pub struct GuiCallbacks {
    pub on_test: extern "C" fn(),
    pub on_update_timeline: extern "C" fn(timeline_type: usize),
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    set_responce_callbacks(on_responce_recveve);
}

#[unsafe(no_mangle)]
pub extern "C" fn set_gui_callbacks(callbacks: GuiCallbacks) {
    GUI_CALLBACKS.set(callbacks).ok();
}

fn update_timeline(timeline_type: usize) {
    if let Some(cb) = GUI_CALLBACKS.get() {
        (cb.on_update_timeline)(timeline_type);
    }
}
