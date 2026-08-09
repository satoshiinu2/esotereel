use std::{
    os::raw::c_void,
    sync::{Arc, RwLockReadGuard},
};

use esotereel_lib::project::{Project, Timeline, ids::TimelineId};

use crate::{WrapperErrorCode, wrapper::network::ProjectReadGuard};

pub mod clip;
pub mod clip_render_info;
pub mod debug;
pub mod layer;
pub mod timeline;

#[unsafe(no_mangle)]
pub extern "C" fn project_get_timeline(ptr: *const Project, id: TimelineId) -> *const Timeline {
    if ptr.is_null() {
        return std::ptr::null();
    }
    let project = unsafe { &(*ptr) };

    project
        .timeline(id)
        .map(|t| t as *const _)
        .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn project_get_timeline_count(ptr: *const Project) -> usize {
    if ptr.is_null() {
        return 0;
    }
    let project = unsafe { &(*ptr) };

    project.timeline_count()
}

/// ガード（guard_ptr）から *const Project を安全に取り出す関数
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_guard_get_project_from_guard(
    guard_ptr: *const c_void,
    out_project: *mut *const Project,
) -> WrapperErrorCode {
    if guard_ptr.is_null() || out_project.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let guard = unsafe { &*(guard_ptr as *const ProjectReadGuard<'static>) };
    unsafe { *out_project = guard.project_ptr };

    WrapperErrorCode::ok()
}
