use esotereel_lib::project::{Project, timeline::Timeline};

pub mod clip;
pub mod layer;
pub mod timeline;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_timeline(ptr: *const Project, id: usize) -> *const Timeline {
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
pub unsafe extern "C" fn project_get_timeline_count(ptr: *const Project) -> usize {
    unsafe { (*ptr).get_timeline_count() }
}
