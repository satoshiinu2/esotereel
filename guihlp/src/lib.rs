use std::sync::OnceLock;

pub use esotereel_lib::decode::streamplayer::StreamPlayer;
pub use esotereel_lib::project::Project;
pub use esotereel_lib::project::clip::Clip;
pub use esotereel_lib::project::layer::Layer;
pub use esotereel_lib::project::timeline::Timeline;

use crate::network::OnConnectedFn;
use crate::responces::on_responce_recveve;
use crate::wrapper::stringview::StringView;

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
pub struct WrapperResult {
    pub code: WrapperErrorCode,
    pub message: StringView,
}

impl WrapperResult {
    pub fn ok() -> Self {
        Self {
            code: WrapperErrorCode::Ok,
            message: StringView::zero(),
        }
    }
    pub fn null_ptr() -> Self {
        Self {
            code: WrapperErrorCode::NullPtr,
            message: StringView::zero(),
        }
    }
    pub fn not_found(message: Option<&str>) -> Self {
        Self {
            code: WrapperErrorCode::NotFound,
            message: StringView::from_option_str(message),
        }
    }
    pub fn error(message: Option<&str>) -> Self {
        Self {
            code: WrapperErrorCode::Error,
            message: StringView::from_option_str(message),
        }
    }
    pub fn panic(message: Option<&str>) -> Self {
        Self {
            code: WrapperErrorCode::Panic,
            message: StringView::from_option_str(message),
        }
    }

    pub fn invalid_string_error() -> Self {
        Self {
            code: WrapperErrorCode::Error,
            message: StringView::from_str("invalid string"),
        }
    }
}

#[repr(C)]
pub struct GuiCallbacks {
    pub on_test: extern "C" fn(),
    pub redraw_timeline: extern "C" fn(timeline_type: usize),
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn init() {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_gui_callbacks(callbacks: GuiCallbacks) {
    GUI_CALLBACKS.set(callbacks).ok();
}

fn update_timeline(timeline_type: usize) {
    if let Some(cb) = GUI_CALLBACKS.get() {
        (cb.redraw_timeline)(timeline_type);
    }
}
