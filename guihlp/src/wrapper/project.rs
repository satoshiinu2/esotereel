use nomyoedit_lib::project::{Project, timeline::Timeline};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_timeline(ptr: *const Project, idx: usize) -> *const Timeline {
    unsafe { &(*ptr).timelines[idx] }
}
