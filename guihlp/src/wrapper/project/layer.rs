use esotereel_lib::project::{Clip, Layer};

use crate::{WrapperErrorCode, wrapper::stringview::StringView};

pub struct ClipIterator<'a> {
    // どんなイテレータでも保持できるように dyn Iterator にする
    inner: Box<dyn Iterator<Item = &'a Clip> + 'a>,
    parent: &'a Layer,
}

#[unsafe(no_mangle)]
pub extern "C" fn layer_find_clip_at_frame(
    ptr: *const Layer,
    frame: i64,
    out: *mut *const Clip,
) -> WrapperErrorCode {
    if ptr.is_null() || out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let layer = unsafe { &(*ptr) };
    let clip = layer.clips.get_at(frame);

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

#[unsafe(no_mangle)]
pub extern "C" fn layer_clips_begin(
    layer: &Layer,
    out: *mut *mut ClipIterator<'static>,
) -> WrapperErrorCode {
    if out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let iter = Box::new(ClipIterator {
        inner: Box::new(layer.clips.iter()),
        parent: layer,
    });

    unsafe {
        let raw_iter = Box::into_raw(iter);
        *out = std::mem::transmute::<*mut ClipIterator<'_>, *mut ClipIterator<'static>>(raw_iter);
    }
    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn layer_clips_in_range_begin(
    layer: &Layer,
    start: i64,
    end: i64,
    out: *mut *mut ClipIterator<'static>,
) -> WrapperErrorCode {
    if out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let iter = Box::new(ClipIterator {
        inner: Box::new(layer.clips.range(start..end)),
        parent: layer,
    });

    unsafe {
        let raw_iter = Box::into_raw(iter);
        *out = std::mem::transmute::<*mut ClipIterator<'_>, *mut ClipIterator<'static>>(raw_iter);
    }
    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn clip_iter_next<'a>(
    iter_ptr: *mut ClipIterator<'a>,
    out: *mut *const Clip,
) -> WrapperErrorCode {
    if iter_ptr.is_null() || out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let iter = unsafe { &mut *iter_ptr };

    let next = iter.inner.next();

    match next {
        Some(clip) => {
            unsafe { *out = clip as *const Clip };
            WrapperErrorCode::ok()
        }
        None => {
            unsafe { *out = std::ptr::null() };
            WrapperErrorCode::not_found(Some("next clip"))
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn clip_iter_free(iter: *mut ClipIterator) -> WrapperErrorCode {
    if iter.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    unsafe {
        drop(Box::from_raw(iter));
    };

    WrapperErrorCode::ok()
}
