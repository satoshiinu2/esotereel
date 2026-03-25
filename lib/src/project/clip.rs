use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Clone)]
pub struct Clip {
    pub id: u32,
    pub position: i64,
    pub duration: i64,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_get_id(ptr: *const Clip) -> u32 {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).id }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_get_position(ptr: *const Clip) -> i64 {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).position }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_get_duration(ptr: *const Clip) -> i64 {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).duration }
}
