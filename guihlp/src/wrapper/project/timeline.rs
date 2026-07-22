use esotereel_lib::project::{clip::Clip, layer::Layer, timeline::Timeline};

use crate::{WrapperErrorCode, slice_from_ptr_safe};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layer_by_order(
    ptr: *const Timeline,
    order: u32,
) -> *const Layer {
    if ptr.is_null() {
        return std::ptr::null();
    }

    let layers = unsafe { &(*ptr).layers };
    if let Some(layer) = layers.get_by_sorted_idx(order) {
        layer.as_ref() as *const Layer
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
    out_layer_idx: *mut u32,
) -> WrapperErrorCode {
    if ptr.is_null() | out_clip.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let project = unsafe { &(*ptr) };

    let Some((_, clip, layer_idx)) = project.layers.find_orderd_clip_by_id(clip_id) else {
        return WrapperErrorCode::not_found(Some("clip not found"));
    };
    unsafe {
        *out_clip = clip.as_ref();
        *out_layer_idx = layer_idx;
    };
    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_can_place_clip_at(
    ptr: *const Timeline,
    layer_order: u32,
    position: i64,
    duration: i64,
    exclude_ids_ptr: *const u64,
    exclude_ids_len: usize,
) -> bool {
    if ptr.is_null() {
        return false;
    }
    let timeline = unsafe { &(*ptr) };

    let exclude_ids = slice_from_ptr_safe(exclude_ids_ptr, exclude_ids_len);

    let Some(layer_map_key) = timeline.layers.get_layer_map_key_by_order(layer_order) else {
        return false;
    };

    timeline
        .layers
        .can_place_clip_at(&layer_map_key, position, duration, exclude_ids)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_fps(ptr: *const Timeline) -> f64 {
    if ptr.is_null() {
        return f64::NAN;
    }
    let timeline = unsafe { &(*ptr) };

    timeline.fps
}
