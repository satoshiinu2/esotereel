use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use std::ffi::c_void;
use std::ptr::NonNull;

use crate::render::wgpuutil::{self, WGpuUtil};
