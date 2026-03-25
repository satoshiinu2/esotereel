use crate::project::clip::Clip;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Clone)]
pub struct Layer {
    pub index: usize,
    pub clips: Vec<Clip>,
    pub name: String,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn layer_get_clip(ptr: *const Layer, l_idx: usize) -> *const Clip {
    if ptr.is_null() {
        return std::ptr::null();
    }

    unsafe { std::ptr::addr_of!((&(*ptr).clips)[l_idx]) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn layer_get_clips_count(ptr: *const Layer) -> usize {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).clips.len() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn layer_get_name_ptr(ptr: *const Layer) -> *const u8 {
    if ptr.is_null() {
        return std::ptr::null();
    }
    unsafe { (*ptr).name.as_ptr() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn layer_get_name_len(ptr: *const Layer) -> usize {
    if ptr.is_null() {
        return 0;
    }
    unsafe { (&(*ptr).name).len() }
}
