use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::render::wgpuutil::{OffscreenTarget, WGpuUtil};

use crate::{WrapperErrorCode, wrapper::log_if_panicked};

#[unsafe(no_mangle)]
pub extern "C" fn wgpuutil_new(
    width: u32,
    height: u32,
    out: *mut *mut WGpuUtil,
) -> WrapperErrorCode {
    log::debug!(
        "wgpu init (offscreen), width: {}, height: {}",
        width,
        height
    );

    let result = catch_unwind(AssertUnwindSafe(|| {
        let wgpuutil = WGpuUtil::new(width, height);
        unsafe { *out = Box::into_raw(Box::new(wgpuutil)) }
    }));

    let msg = log_if_panicked(result, "wgpuutil_new");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_drop(ptr: *mut WGpuUtil) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        unsafe { drop(Box::from_raw(ptr)) };
    }));
    let msg = log_if_panicked(result, "wgpuutil_drop");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub extern "C" fn offscreen_target_new(
    ptr_wgpu: *mut WGpuUtil,
    width: u32,
    height: u32,
    out: *mut *mut OffscreenTarget,
) -> WrapperErrorCode {
    if ptr_wgpu.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let util = unsafe { &*ptr_wgpu };
        let target = OffscreenTarget::new(&util.device, util.format, width, height);
        unsafe { *out = Box::into_raw(Box::new(target)) }
    }));
    let msg = log_if_panicked(result, "offscreen_target_new");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn offscreen_target_drop(ptr: *mut OffscreenTarget) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        unsafe { drop(Box::from_raw(ptr)) };
    }));
    let msg = log_if_panicked(result, "offscreen_target_drop");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_free_buffer(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe { drop(Vec::from_raw_parts(ptr, len, len)) };
    }
}
