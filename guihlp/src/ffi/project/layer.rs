use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    project::{Clip, Layer, Timeline, layer_outline::LayerFolder},
    util::result::EsotereelError,
};

use crate::ffi::{result::FfiResult, stringview::FfiStringView};

#[unsafe(no_mangle)]
pub extern "C" fn layer_find_clip_at_frame(
    layer_ptr: *const Layer,
    timeline_ptr: *const Timeline,
    frame: i64,
) -> FfiResult<*const Clip> {
    fn inner(
        layer_ptr: *const Layer,
        timeline_ptr: *const Timeline,
        frame: i64,
    ) -> anyhow::Result<*const Clip> {
        if layer_ptr.is_null() || timeline_ptr.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("layer_ptr".to_string()));
        }
        if timeline_ptr.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("timeline_ptr".to_string()));
        }

        let layer = unsafe { &(*layer_ptr) };
        let timeline = unsafe { &(*timeline_ptr) };

        let clip_id = layer.get_clip_id_at(frame);

        let Some(clip_id) = clip_id else {
            anyhow::bail!(EsotereelError::ClipNotFoundAt(frame));
        };

        let clip = timeline.get_clip(clip_id);

        let Some(clip) = clip else {
            anyhow::bail!(EsotereelError::ClipNotFound(clip_id));
        };

        Ok(clip as *const Clip)
    }

    FfiResult::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(layer_ptr, timeline_ptr, frame)
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn layer_get_clips_count(ptr: *const Layer) -> usize {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).clips.len() }
}

// Layerのライフタイム内ならStringViewは有効
#[unsafe(no_mangle)]
pub extern "C" fn layer_get_name(ptr: *const Layer) -> FfiResult<FfiStringView> {
    if ptr.is_null() {
        return FfiResult::err(EsotereelError::NullPointer("layer_ptr".to_string()).into());
    }
    unsafe { FfiResult::ok(FfiStringView::from_str(&(*ptr).name)) }
}

// のライフタイム内ならStringViewは有効
#[unsafe(no_mangle)]
pub extern "C" fn layer_folder_get_name(ptr: *const LayerFolder) -> FfiResult<FfiStringView> {
    if ptr.is_null() {
        return FfiResult::err(EsotereelError::NullPointer("layer_folder_ptr".to_string()).into());
    }
    unsafe { FfiResult::ok(FfiStringView::from_str(&(*ptr).name)) }
}

// Index-based clip access - replaces iterator pattern
#[unsafe(no_mangle)]
pub extern "C" fn layer_get_clip_at_index(
    layer_ptr: *const Layer,
    timeline_ptr: *const Timeline,
    index: usize,
) -> FfiResult<*const Clip> {
    fn inner(
        layer_ptr: *const Layer,
        timeline_ptr: *const Timeline,
        index: usize,
    ) -> anyhow::Result<*const Clip> {
        if layer_ptr.is_null() || timeline_ptr.is_null() {
            anyhow::bail!(EsotereelError::NullPointer(
                "One or more pointers are null".to_string()
            ));
        }

        let layer = unsafe { &(*layer_ptr) };
        let timeline = unsafe { &(*timeline_ptr) };

        let clip_id = layer.clips.iter().nth(index).map(|(_, &id)| id);

        let Some(clip_id) = clip_id else {
            anyhow::bail!(EsotereelError::ClipNotFoundAt(index as i64));
        };

        let clip = timeline.get_clip(clip_id);

        let Some(clip) = clip else {
            anyhow::bail!(EsotereelError::ClipNotFound(clip_id));
        };

        Ok(clip as *const Clip)
    }

    FfiResult::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(layer_ptr, timeline_ptr, index)
    })))
}

#[unsafe(no_mangle)]
pub extern "C" fn layer_get_clip_at_position(
    layer_ptr: *const Layer,
    timeline_ptr: *const Timeline,
    position: i64,
) -> FfiResult<*const Clip> {
    fn inner(
        layer_ptr: *const Layer,
        timeline_ptr: *const Timeline,
        position: i64,
    ) -> anyhow::Result<*const Clip> {
        if layer_ptr.is_null() || timeline_ptr.is_null() {
            anyhow::bail!(EsotereelError::NullPointer(
                "One or more pointers are null".to_string()
            ));
        }

        let layer = unsafe { &(*layer_ptr) };
        let timeline = unsafe { &(*timeline_ptr) };

        let clip_id = layer.get_clip_id_at(position);

        let Some(clip_id) = clip_id else {
            anyhow::bail!(EsotereelError::ClipNotFoundAt(position));
        };

        let clip = timeline.get_clip(clip_id);

        let Some(clip) = clip else {
            anyhow::bail!(EsotereelError::ClipNotFound(clip_id));
        };

        Ok(clip as *const Clip)
    }

    FfiResult::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(layer_ptr, timeline_ptr, position)
    })))
}
