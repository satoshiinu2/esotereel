use esotereel_lib::project::{clip::Clip, layer::Layer, timeline::Timeline};

use crate::WrapperErrorCode;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layer_at(ptr: *const Timeline, l_idx: usize) -> *const Layer {
    if ptr.is_null() {
        return std::ptr::null();
    }

    let layers = unsafe { &(*ptr).layers };
    if l_idx < layers.len() {
        std::ptr::addr_of!(layers[l_idx])
    } else {
        std::ptr::null()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layers_count(ptr: *const Timeline) -> usize {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).layers.len() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_find_clip_by_id(
    ptr: *const Timeline,
    clip_id: u64,
    out_clip: *mut *const Clip,
    out_layer_idx: *mut usize,
) -> WrapperErrorCode {
    if ptr.is_null() | out_clip.is_null() {
        return WrapperErrorCode::NullPtr;
    }

    let Some((layer_id, clip)) = (unsafe { (*ptr).find_clip_by_id(clip_id) }) else {
        return WrapperErrorCode::NullPtr;
    };
    unsafe {
        *out_clip = clip.as_ref();
        *out_layer_idx = layer_id;
    };
    WrapperErrorCode::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_can_place_clip_at(
    ptr: *const Timeline,
    layer_idx: usize,
    position: i64,
    duration: i64,
    exclude_ids_ptr: *const u64,
    exclude_ids_len: usize,
) -> bool {
    if ptr.is_null() {
        return false;
    }
    let timeline = unsafe { &(*ptr) };

    let exclude_ids = unsafe { std::slice::from_raw_parts(exclude_ids_ptr, exclude_ids_len) };

    timeline.can_place_clip_at(layer_idx, position, duration, exclude_ids)
}
