use esotereel_lib::project::{Project, Timeline, ids::TimelineId};

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
