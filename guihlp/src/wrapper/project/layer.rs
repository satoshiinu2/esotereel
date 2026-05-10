use std::sync::Arc;

use esotereel_lib::project::{clip::Clip, layer::Layer};

use crate::{WrapperErrorCode, wrapper::stringview::StringView};

pub enum ClipIteratorInner<'a> {
    Iter(std::collections::btree_map::Iter<'a, i64, Arc<Clip>>),
    Range(std::collections::btree_map::Range<'a, i64, Arc<Clip>>),
}

impl<'a> ClipIteratorInner<'a> {
    pub fn next(&mut self) -> Option<&Arc<Clip>> {
        match self {
            ClipIteratorInner::Iter(i) => i.next().map(|(_, clip)| clip),
            ClipIteratorInner::Range(i) => i.next().map(|(_, clip)| clip),
        }
    }
}

pub struct ClipIterator<'a> {
    inner: ClipIteratorInner<'a>,
    parent: &'a Layer,
}

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
    let clip = layer.clips.get_at(frame);

    let Some(clip) = clip else {
        return WrapperErrorCode::NotFound;
    };

    unsafe { *out = clip.as_ref() };
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

    let iter = Box::new(ClipIterator {
        inner: ClipIteratorInner::Iter(layer.clips.into_iter()),
        parent: layer,
    });

    unsafe {
        // 寿命の強制変換
        let raw_iter = Box::into_raw(iter);
        *out = std::mem::transmute::<*mut ClipIterator<'_>, *mut ClipIterator<'static>>(raw_iter);

        // 参照を増やしてuaf防止
        Arc::increment_strong_count(layer);
    }
    WrapperErrorCode::Ok
}
#[unsafe(no_mangle)]
pub extern "C" fn layer_clips_in_range_begin(
    layer: &Layer,
    start: i64,
    end: i64,
    out: *mut *mut ClipIterator<'static>,
) -> WrapperErrorCode {
    if out.is_null() {
        return WrapperErrorCode::NullPtr;
    }

    let range = start..end;
    let iter = Box::new(ClipIterator {
        inner: ClipIteratorInner::Range(layer.clips.range(range)),
        parent: layer,
    });

    unsafe {
        // 寿命の強制変換
        let raw_iter = Box::into_raw(iter);
        *out = std::mem::transmute::<*mut ClipIterator<'_>, *mut ClipIterator<'static>>(raw_iter);

        // 参照を増やしてuaf防止
        Arc::increment_strong_count(layer);
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

    let next = iter.inner.next();

    match next {
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
    unsafe {
        // 参照を減らす
        Arc::decrement_strong_count((*iter).parent);
        drop(Box::from_raw(iter));
    };

    WrapperErrorCode::Ok
}
