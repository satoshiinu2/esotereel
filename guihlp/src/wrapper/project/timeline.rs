use esotereel_lib::project::{clip::Clip, layer::Layer, timeline::Timeline};

use crate::WrapperErrorCode;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layer_by_layer_handle(
    ptr: *const Timeline,
    layer_handle: u32,
) -> *const Layer {
    if ptr.is_null() {
        return std::ptr::null();
    }

    let layers = unsafe { &(*ptr).layers };
    if (layer_handle as usize) < layers.len() {
        if let Some(layer) = layers.get_by_layer_handle(layer_handle) {
            layer.as_ref() as *const Layer
        } else {
            std::ptr::null()
        }
    } else {
        std::ptr::null()
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layer_by_sorted_idx(
    ptr: *const Timeline,
    index: u32,
) -> *const Layer {
    if ptr.is_null() {
        return std::ptr::null();
    }

    let layers = unsafe { &(*ptr).layers };
    if (index as usize) < layers.len() {
        if let Some(layer) = layers.get_by_sorted_idx(index) {
            layer.as_ref() as *const Layer
        } else {
            std::ptr::null()
        }
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

    let Some((_, clip, layer_idx)) = (unsafe { (*ptr).find_clip_by_id(clip_id) }) else {
        return WrapperErrorCode::NullPtr;
    };
    unsafe {
        *out_clip = clip.as_ref();
        *out_layer_idx = layer_idx;
    };
    WrapperErrorCode::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_can_place_clip_at(
    ptr: *const Timeline,
    layer_idx: u32,
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
