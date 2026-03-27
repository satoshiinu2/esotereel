use nomyoedit_lib::project::{Project, timeline::Timeline};

pub mod clip;
pub mod layer;
pub mod timeline;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_timeline(ptr: *const Project, idx: usize) -> *const Timeline {
    unsafe { &(*ptr).timelines[idx] }
}
