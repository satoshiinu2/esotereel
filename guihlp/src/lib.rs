use std::sync::OnceLock;

pub use esotereel_lib::decode::streamplayer::StreamPlayer;
pub use esotereel_lib::project::Project;
pub use esotereel_lib::project::clip::Clip;
pub use esotereel_lib::project::layer::Layer;
pub use esotereel_lib::project::timeline::Timeline;

use crate::network::OnConnectedFn;
use crate::responces::on_responce_recveve;

mod network;
pub mod project;
pub mod responces;
pub mod wrapper;

static GUI_CALLBACKS: OnceLock<GuiCallbacks> = OnceLock::new();
static ON_CONNECTED_CALLBACKS: OnceLock<OnConnectedFn> = OnceLock::new();

#[repr(C)]
pub enum WrapperErrorCode {
    Ok = 0,
    NullPtr = 1,
    NotFound = 2,
    Error = 3,
    Panic = 4,
}
#[repr(C)]
pub struct GuiCallbacks {
    pub on_test: extern "C" fn(),
    pub redraw_timeline: extern "C" fn(timeline_type: usize),
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn init() {
    // set_responce_callbacks(on_responce_recveve);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_gui_callbacks(callbacks: GuiCallbacks) {
    GUI_CALLBACKS.set(callbacks).ok();
}

fn update_timeline(timeline_type: usize) {
    if let Some(cb) = GUI_CALLBACKS.get() {
        (cb.redraw_timeline)(timeline_type);
    }
}
