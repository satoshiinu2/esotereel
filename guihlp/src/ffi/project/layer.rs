use esotereel_lib::project::{Clip, Layer, Timeline};

use crate::{WrapperErrorCode, ffi::stringview::StringView};

#[unsafe(no_mangle)]
pub extern "C" fn layer_find_clip_at_frame(
    layer_ptr: *const Layer,
    timeline_ptr: *const Timeline,
    frame: i64,
    out: *mut *const Clip,
) -> WrapperErrorCode {
    if layer_ptr.is_null() || timeline_ptr.is_null() || out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let layer = unsafe { &(*layer_ptr) };
    let timeline = unsafe { &(*timeline_ptr) };
    
    let clip_id = layer.get_clip_id_at(frame);

    let Some(clip_id) = clip_id else {
        return WrapperErrorCode::not_found(Some("clip"));
    };

    let clip = timeline.get_clip(clip_id);

    let Some(clip) = clip else {
        return WrapperErrorCode::not_found(Some("clip"));
    };

    unsafe { *out = clip as *const Clip };
    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn layer_get_clips_count(ptr: *const Layer) -> usize {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).clips.len() }
}

#[unsafe(no_mangle)]
pub extern "C" fn layer_get_name(ptr: *const Layer) -> StringView {
    if ptr.is_null() {
        return StringView::zero();
    }
    unsafe { StringView::from_str(&(*ptr).name) }
}

// Index-based clip access - replaces iterator pattern
#[unsafe(no_mangle)]
pub extern "C" fn layer_get_clip_at_index(
    layer_ptr: *const Layer,
    timeline_ptr: *const Timeline,
    index: usize,
    out: *mut *const Clip,
) -> WrapperErrorCode {
    if layer_ptr.is_null() || timeline_ptr.is_null() || out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let layer = unsafe { &(*layer_ptr) };
    let timeline = unsafe { &(*timeline_ptr) };

    let clip_id = layer.clips.iter().nth(index).map(|(_, &id)| id);

    let Some(clip_id) = clip_id else {
        return WrapperErrorCode::not_found(Some("clip index out of bounds"));
    };

    let clip = timeline.get_clip(clip_id);

    let Some(clip) = clip else {
        return WrapperErrorCode::not_found(Some("clip not found in timeline"));
    };

    unsafe { *out = clip as *const Clip };
    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn layer_get_clip_at_position(
    layer_ptr: *const Layer,
    timeline_ptr: *const Timeline,
    position: i64,
    out: *mut *const Clip,
) -> WrapperErrorCode {
    if layer_ptr.is_null() || timeline_ptr.is_null() || out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let layer = unsafe { &(*layer_ptr) };
    let timeline = unsafe { &(*timeline_ptr) };

    let clip_id = layer.get_clip_id_at(position);

    let Some(clip_id) = clip_id else {
        return WrapperErrorCode::not_found(Some("clip not found at position"));
    };

    let clip = timeline.get_clip(clip_id);

    let Some(clip) = clip else {
        return WrapperErrorCode::not_found(Some("clip not found in timeline"));
    };

    unsafe { *out = clip as *const Clip };
    WrapperErrorCode::ok()
}
