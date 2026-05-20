use std::{
    os::raw::c_void,
    sync::{Arc, RwLockReadGuard},
};

use esotereel_lib::project::{Project, timeline::Timeline};

use crate::WrapperResult;

pub mod clip;
pub mod debug;
pub mod layer;
pub mod timeline;

#[unsafe(no_mangle)]
pub extern "C" fn project_get_timeline(ptr: *const Project, id: usize) -> *const Timeline {
    if ptr.is_null() {
        return std::ptr::null();
    }

    unsafe {
        (*ptr)
            .get_timeline(id)
            .map(|t| t as *const Timeline)
            .unwrap_or(std::ptr::null())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn project_get_timeline_count(ptr: *const Project) -> usize {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).get_timeline_count() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_guard_get_project_from_guard(
    guard_ptr: *const c_void,
    out: *mut *const Project,
) -> WrapperResult {
    if guard_ptr.is_null() || out.is_null() {
        return WrapperResult::null_ptr();
    }

    let guard = unsafe { &*(guard_ptr as *const RwLockReadGuard<Option<Arc<Project>>>) };

    // ガードの中身を覗き見して、Projectの生ポインタを返す
    match guard.as_ref() {
        Some(p) => {
            unsafe { *out = Arc::as_ptr(p) };
            WrapperResult::ok()
        }
        None => {
            unsafe { *out = std::ptr::null() };
            WrapperResult::not_found(Some("project not found"))
        }
    }
}
