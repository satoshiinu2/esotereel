use nomyoedit_lib::{
    project::{clip::Clip, layer::Layer},
    types::ClipLocation,
};

pub struct ClipIterator<'a>(std::collections::btree_set::Iter<'a, Clip>);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn layer_find_clip_at_frame(
    ptr: *const Layer,
    frame: u64,
    layer_idx: usize,
) -> ClipLocation {
    let layer = unsafe { &(*ptr) };

    if !ptr.is_null() {
        for (clip_idx, clip) in layer.clips.iter().enumerate() {
            if frame >= clip.position && frame < clip.position + clip.duration {
                return ClipLocation {
                    layer_idx,
                    clip_idx,
                    clip,
                };
            }
        }
    }
    return ClipLocation {
        layer_idx: 0,
        clip_idx: 0,
        clip: std::ptr::null(),
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn layer_get_clip_at_slow(ptr: *const Layer, idx: usize) -> *const Clip {
    if ptr.is_null() {
        return std::ptr::null();
    }

    let layer = unsafe { &*ptr };

    // nth(idx) で要素を探し、あればその参照をポインタとして返す
    match layer.clips.iter().nth(idx) {
        Some(clip_ref) => clip_ref as *const Clip,
        None => std::ptr::null(),
    }
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

#[unsafe(no_mangle)]
pub extern "C" fn layer_clips_begin(layer: &Layer) -> *mut ClipIterator<'_> {
    let iter = Box::new(ClipIterator(layer.clips.iter()));
    Box::into_raw(iter)
}

#[unsafe(no_mangle)]
pub extern "C" fn clip_iter_next<'a>(iter: &mut ClipIterator<'a>) -> *const Clip {
    iter.0
        .next()
        .map(|c| c as *const Clip)
        .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn clip_iter_free(iter: *mut ClipIterator) {
    if !iter.is_null() {
        unsafe {
            drop(Box::from_raw(iter));
        }
    }
}
