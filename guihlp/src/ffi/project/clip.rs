use esotereel_lib::project::{Clip, TimelineTick, ids::ClipId};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_get_id(ptr: *const Clip) -> ClipId {
    if ptr.is_null() {
        return 0;
    }
    let clip = unsafe { &*ptr };
    clip.id
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_get_position(ptr: *const Clip) -> TimelineTick {
    if ptr.is_null() {
        return 0;
    }
    let clip = unsafe { &*ptr };
    clip.position
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_get_duration(ptr: *const Clip) -> TimelineTick {
    if ptr.is_null() {
        return 0;
    }
    let clip = unsafe { &*ptr };
    clip.duration
}
