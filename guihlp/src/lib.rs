use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::{CString, c_char};
use std::sync::OnceLock;

pub use esotereel_lib::decode::streamplayer::StreamPlayer;
pub use esotereel_lib::project::Layer;
pub use esotereel_lib::project::Project;
pub use esotereel_lib::project::Timeline;
pub use esotereel_lib::project::clip::Clip;
pub use esotereel_lib::project::ids::{ClipId, LayerId, ScriptId, TimelineId};
pub use esotereel_lib::render::surfacetarget::NativeWindowHandle;
use log::error;

use crate::network::OnConnectedFn;
use crate::responces::on_responce_recveve;

mod network;
pub mod responces;
pub mod wrapper;

static GUI_CALLBACKS: OnceLock<GuiCallbacks> = OnceLock::new();
static ON_CONNECTED_CALLBACKS: OnceLock<OnConnectedFn> = OnceLock::new();

thread_local! {
    static LAST_ERR_MSG:RefCell<CString>=RefCell::new(CString::new("").unwrap());
}

pub enum IntoWrapperError<'a> {
    Ok,
    NullPtr,
    NotFound(Option<Cow<'a, str>>),
    Error(Option<Cow<'a, str>>),
    Panic(Option<Cow<'a, str>>),
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

        if let Err(e) = LAST_ERR_MSG.try_with(|e| {
            *e.borrow_mut() = c_str;
        }) {
            if let Some(message) = message {
                error!("AccessError: maybe recieve error on exiting: {e} msg: {message}");
            }
        }
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

    /// anyhow::ErrorからWrapperErrorCodeへの変換
    pub fn from_anyhow(err: anyhow::Error) -> Self {
        let error_msg = err.to_string();

        // エラーの種類に基づいて分類
        if error_msg.contains("not found") || error_msg.contains("NotFound") {
            Self::not_found(Some(&error_msg))
        } else if error_msg.contains("null") || error_msg.contains("NullPtr") {
            Self::null_ptr()
        } else if error_msg.contains("panic") || error_msg.contains("Panic") {
            Self::panic(Some(&error_msg))
        } else {
            Self::error(Some(&error_msg))
        }
    }
}

impl<'a> IntoWrapperError<'a> {
    pub fn set_last_err_msg(&self) {
        match self {
            IntoWrapperError::Ok => WrapperErrorCode::set_last_err_msg(None),
            IntoWrapperError::NullPtr => WrapperErrorCode::set_last_err_msg(None),
            IntoWrapperError::NotFound(e) => WrapperErrorCode::set_last_err_msg(e.as_deref()),
            IntoWrapperError::Error(e) => WrapperErrorCode::set_last_err_msg(e.as_deref()),
            IntoWrapperError::Panic(e) => WrapperErrorCode::set_last_err_msg(e.as_deref()),
        }
    }
}

impl<'a> From<IntoWrapperError<'a>> for WrapperErrorCode {
    fn from(e: IntoWrapperError) -> Self {
        match e {
            IntoWrapperError::Ok => WrapperErrorCode::Ok,
            IntoWrapperError::NullPtr => WrapperErrorCode::NullPtr,
            IntoWrapperError::NotFound(_) => WrapperErrorCode::NotFound,
            IntoWrapperError::Error(_) => WrapperErrorCode::Error,
            IntoWrapperError::Panic(_) => WrapperErrorCode::Panic,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_last_err_msg() -> *const c_char {
    LAST_ERR_MSG.with(|e| e.borrow().as_ptr())
}

#[repr(C)]
pub struct GuiCallbacks {
    pub on_test: extern "C" fn(),
    pub mark_dirty_timeline: extern "C" fn(timeline_type: TimelineId),
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn init() {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_gui_callbacks(callbacks: GuiCallbacks) {
    GUI_CALLBACKS.set(callbacks).ok();
}

pub fn mark_dirty_timeline(timeline_type: TimelineId) {
    if let Some(cb) = GUI_CALLBACKS.get() {
        (cb.mark_dirty_timeline)(timeline_type);
    }
}

/// # Safety
///
/// If `len > 0` and `ptr` is non-null, `ptr` must point to a valid
/// contiguous array of at least `len` initialized `T`s, and the memory
/// must remain valid for the returned slice's lifetime.
pub unsafe fn slice_from_ptr_or_empty<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}
