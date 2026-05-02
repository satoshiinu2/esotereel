use std::sync::OnceLock;

pub use esotereel_lib::decode::videoreciever::StreamReciever;
pub use esotereel_lib::project::Project;
pub use esotereel_lib::project::clip::Clip;
pub use esotereel_lib::project::layer::Layer;
pub use esotereel_lib::project::timeline::Timeline;
use esotereel_lib::responces::set_responce_callbacks;

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
    pub on_update_timeline: extern "C" fn(timeline_type: usize),
    // デコードされたフレームをGUIに渡すためのコールバック
    pub on_stream_frame: extern "C" fn(resource_id: u32, width: u32, height: u32, data: *const u8),
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn init() {
    set_responce_callbacks(on_responce_recveve);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_gui_callbacks(callbacks: GuiCallbacks) {
    GUI_CALLBACKS.set(callbacks).ok();
}

fn update_timeline(timeline_type: usize) {
    if let Some(cb) = GUI_CALLBACKS.get() {
        (cb.on_update_timeline)(timeline_type);
    }
}

pub(crate) fn update_stream_frame(resource_id: u32, width: u32, height: u32, data: *const u8) {
    if let Some(cb) = GUI_CALLBACKS.get() {
        (cb.on_stream_frame)(resource_id, width, height, data);
    }
}
