use std::{
    os::raw::c_void,
    sync::{Arc, RwLockReadGuard},
};

use esotereel_lib::project::{Project, timeline::Timeline};

use crate::WrapperErrorCode;

pub mod clip;
pub mod clip_render_info;
pub mod debug;
pub mod layer;
pub mod timeline;

#[unsafe(no_mangle)]
pub extern "C" fn project_get_timeline(ptr: *const Project, id: usize) -> *const Timeline {
    if ptr.is_null() {
        return std::ptr::null();
    }
    let project = unsafe { &(*ptr) };
    let key = project.timelines.get_cureent_new_key(id);

    project
        .timelines
        .get(&key)
        .map(|t| t as *const Timeline)
        .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn project_get_timeline_count(ptr: *const Project) -> usize {
    if ptr.is_null() {
        return 0;
    }
    let project = unsafe { &(*ptr) };

    project.timelines.len()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_guard_get_project_from_guard(
    guard_ptr: *const c_void,
    out: *mut *const Project,
) -> WrapperErrorCode {
    if guard_ptr.is_null() || out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let guard = unsafe { &*(guard_ptr as *const RwLockReadGuard<Option<Arc<Project>>>) };

    // ガードの中身を覗き見して、Projectの生ポインタを返す
    match guard.as_ref() {
        Some(p) => {
            unsafe { *out = Arc::as_ptr(p) };
            WrapperErrorCode::ok()
        }
        None => {
            unsafe { *out = std::ptr::null() };
            WrapperErrorCode::not_found(Some("project not found"))
        }
    }
}
