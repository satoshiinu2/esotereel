use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
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

thread_local! {
    static LAST_ERR_MSG:RefCell<CString>=RefCell::new(CString::new("").unwrap());
}

#[repr(C)]
pub enum WrapperErrorCode {
    Ok = 0,
    NullPtr = 1,
    NotFound = 2,
    Error = 3,
    Panic = 4,
}

impl WrapperErrorCode {
    pub fn set_last_err_msg(message: Option<&str>) {
        let c_str = message
            .and_then(|msg| CString::new(msg).ok())
            .unwrap_or_default();

        LAST_ERR_MSG.with(|e| {
            *e.borrow_mut() = c_str;
        });
    }

    pub fn ok() -> Self {
        Self::set_last_err_msg(None);
        WrapperErrorCode::Ok
    }

    pub fn null_ptr() -> Self {
        Self::set_last_err_msg(Some(""));
        WrapperErrorCode::NullPtr
    }

    pub fn not_found(message: Option<&str>) -> Self {
        Self::set_last_err_msg(message);
        WrapperErrorCode::NotFound
    }

    pub fn error(message: Option<&str>) -> Self {
        Self::set_last_err_msg(message);
        WrapperErrorCode::Error
    }

    pub fn panic(message: Option<&str>) -> Self {
        Self::set_last_err_msg(message);
        WrapperErrorCode::Panic
    }

    pub fn error_from_option(message: Option<&str>) -> Self {
        match message {
            Some(_) => WrapperErrorCode::error(message),
            None => WrapperErrorCode::ok(),
        }
    }

    pub fn invalid_string_error() -> Self {
        Self::set_last_err_msg(Some("invalid string"));
        WrapperErrorCode::Error
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_last_err_msg() -> *const c_char {
    LAST_ERR_MSG.with(|e| e.borrow().as_ptr())
}

#[repr(C)]
pub struct GuiCallbacks {
    pub on_test: extern "C" fn(),
    pub mark_dirty_timeline: extern "C" fn(timeline_type: usize),
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn init() {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_gui_callbacks(callbacks: GuiCallbacks) {
    GUI_CALLBACKS.set(callbacks).ok();
}

pub fn mark_dirty_timeline(timeline_type: usize) {
    if let Some(cb) = GUI_CALLBACKS.get() {
        (cb.mark_dirty_timeline)(timeline_type);
    }
}

pub fn slice_from_ptr_safe<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}
