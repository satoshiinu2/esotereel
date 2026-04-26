use std::sync::Arc;

use esotereel_lib::project::{clip::Clip, layer::Layer};

use crate::{WrapperErrorCode, wrapper::stringview::StringView};

pub struct ClipIterator<'a>(std::collections::btree_set::Iter<'a, Arc<Clip>>);

#[unsafe(no_mangle)]
pub extern "C" fn layer_find_clip_at_frame(
    ptr: *const Layer,
    frame: i64,
    out: *mut *const Clip,
) -> WrapperErrorCode {
    if ptr.is_null() || out.is_null() {
        return WrapperErrorCode::NullPtr;
    }

    let layer = unsafe { &(*ptr) };
    let clip = layer.get_clip_at_frame(frame);
    let Some(clip) = clip else {
        return WrapperErrorCode::NotFound;
    };

    unsafe { *out = clip };
    WrapperErrorCode::Ok
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

#[unsafe(no_mangle)]
pub extern "C" fn layer_clips_begin(
    layer: &Layer,
    out: *mut *mut ClipIterator<'static>,
) -> WrapperErrorCode {
    if out.is_null() {
        return WrapperErrorCode::NullPtr;
    }

    let iter = Box::new(ClipIterator(layer.clips.into_iter()));
    // 寿命の強制変換
    unsafe {
        let raw_iter = Box::into_raw(iter);
        *out = std::mem::transmute::<*mut ClipIterator<'_>, *mut ClipIterator<'static>>(raw_iter);
    }
    WrapperErrorCode::Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn clip_iter_next<'a>(
    iter_ptr: *mut ClipIterator<'a>,
    out: *mut *const Clip,
) -> WrapperErrorCode {
    if iter_ptr.is_null() || out.is_null() {
        return WrapperErrorCode::NullPtr;
    }

    let iter = unsafe { &mut *iter_ptr };

    match iter.0.next() {
        Some(clip) => {
            unsafe { *out = clip.as_ref() };
            WrapperErrorCode::Ok
        }
        None => {
            unsafe { *out = std::ptr::null() };
            WrapperErrorCode::NotFound
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn clip_iter_free(iter: *mut ClipIterator) -> WrapperErrorCode {
    if iter.is_null() {
        return WrapperErrorCode::NullPtr;
    }
    unsafe { drop(Box::from_raw(iter)) };
    WrapperErrorCode::Ok
}
