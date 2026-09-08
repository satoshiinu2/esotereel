use esotereel_lib::project::{
    Clip, Layer, Timeline,
    ids::{ClipId, LayerFolderId, LayerId},
    layer_outline::LayerFolder,
};

use crate::{WrapperErrorCode, slice_from_ptr_or_empty};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_find_clip_by_id(
    ptr: *const Timeline,
    clip_id: ClipId,
    out_clip: *mut *const Clip,
    out_layer_id: *mut LayerId,
) -> WrapperErrorCode {
    if ptr.is_null() || out_clip.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let timeline = unsafe { &(*ptr) };

    let Some((clip, layer_id)) = timeline.get_clip_and_layer(clip_id) else {
        return WrapperErrorCode::not_found(Some("clip not found"));
    };

    unsafe {
        *out_clip = clip;
        if !out_layer_id.is_null() {
            *out_layer_id = layer_id;
        }
    };
    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_can_place_clip_at(
    ptr: *const Timeline,
    layer_id: u64,
    position: i64,
    duration: i64,
    exclude_ids_ptr: *const u64,
    exclude_ids_len: usize,
) -> bool {
    if ptr.is_null() {
        return false;
    }
    let timeline = unsafe { &(*ptr) };

    let exclude_ids = unsafe { slice_from_ptr_or_empty(exclude_ids_ptr, exclude_ids_len) };

    timeline.can_place_clip_at(layer_id, position, duration, exclude_ids)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_fps(ptr: *const Timeline) -> f64 {
    if ptr.is_null() {
        return f64::NAN;
    }
    let timeline = unsafe { &(*ptr) };

    timeline.tps
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layer_by_id(
    ptr: *const Timeline,
    layer_id: LayerId,
) -> *const Layer {
    if ptr.is_null() {
        return std::ptr::null();
    }

    let timeline = unsafe { &(*ptr) };

    timeline
        .get_layer(layer_id)
        .map(|layer| layer as *const Layer)
        .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_folder_by_id(
    ptr: *const Timeline,
    folder_id: LayerFolderId,
) -> *const LayerFolder {
    if ptr.is_null() {
        return std::ptr::null();
    }

    let timeline = unsafe { &(*ptr) };

    timeline
        .get_folder(folder_id)
        .map(|layer| layer as *const LayerFolder)
        .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layer_by_execution_index(
    ptr: *const Timeline,
    index: usize,
) -> *const Layer {
    let Some(timeline) = (unsafe { ptr.as_ref() }) else {
        return std::ptr::null();
    };

    timeline
        .outline
        .iter_execution_order()
        .nth(index)
        .and_then(|id| timeline.get_layer(id))
        .map(|layer| layer as *const Layer)
        .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layers_count(ptr: *const Timeline) -> usize {
    if ptr.is_null() {
        return 0;
    }
    unsafe { (*ptr).outline.iter_execution_order().count() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layer_id_at_execution_index(
    ptr: *const Timeline,
    index: usize,
) -> u64 {
    if ptr.is_null() {
        return 0;
    }
    let timeline = unsafe { &(*ptr) };
    timeline
        .outline
        .iter_execution_order()
        .nth(index)
        .unwrap_or(0)
}
