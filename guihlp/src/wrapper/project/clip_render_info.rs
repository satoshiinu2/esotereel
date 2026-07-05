use std::{collections::HashSet, num::NonZeroU64};

use esotereel_lib::project::{Project, clip_data::ClipData, timeline::Timeline};

use crate::{WrapperResult, slice_from_ptr_safe};

#[repr(C)]
pub struct ClipRenderInfo {
    pub clip_id: u64,
    pub abs_frame: i64,
    pub duration: i64,
    pub is_composite: bool,
    pub is_open: bool,
}

pub struct LayerRow {
    pub layer_order: u32,
    pub depth: u32,
    pub clips: Vec<ClipRenderInfo>,
}

#[repr(C)]
pub struct FfiLayerRow {
    pub layer_order: u32,
    pub depth: u32,
    pub clip_start: u32, // clips配列内の開始インデックス
    pub clip_count: u32,
}

pub struct RenderRowsResult {
    rows: Vec<FfiLayerRow>,
    clips: Vec<ClipRenderInfo>, // 全行ぶんのクリップを1本の配列にまとめたもの
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_rows_build(
    project: *const Project,
    timeline: *const Timeline,
    open_ids_ptr: *const u64,
    open_ids_len: usize,
    out: *mut *mut RenderRowsResult,
) -> WrapperResult {
    if project.is_null() || timeline.is_null() || out.is_null() {
        return WrapperResult::null_ptr();
    }
    let project = unsafe { &*project };
    let timeline = unsafe { &*timeline };
    let open_ids: HashSet<u64> =
        slice_from_ptr_safe(open_ids_ptr, open_ids_len).iter().cloned().collect();

    let mut layer_rows = Vec::new();
    build_layer_rows(project, timeline, &open_ids, 0, 0, &mut layer_rows);

    // フラット化: rows + clips の2本の配列に変換
    let mut rows = Vec::with_capacity(layer_rows.len());
    let mut clips = Vec::new();
    for row in layer_rows {
        let clip_start = clips.len() as u32;
        let clip_count = row.clips.len() as u32;
        clips.extend(row.clips);
        rows.push(FfiLayerRow {
            layer_order: row.layer_order,
            depth: row.depth,
            clip_start,
            clip_count,
        });
    }

    unsafe { *out = Box::into_raw(Box::new(RenderRowsResult { rows, clips })) };
    WrapperResult::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_rows_free(ptr: *mut RenderRowsResult) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_rows_get_rows(
    ptr: *const RenderRowsResult,
    out_ptr: *mut *const FfiLayerRow,
    out_len: *mut usize,
) -> WrapperResult {
    if ptr.is_null() { return WrapperResult::null_ptr(); }
    let result = unsafe { &*ptr };
    unsafe {
        *out_ptr = result.rows.as_ptr();
        *out_len = result.rows.len();
    }
    WrapperResult::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_rows_get_clips(
    ptr: *const RenderRowsResult,
    out_ptr: *mut *const ClipRenderInfo,
    out_len: *mut usize,
) -> WrapperResult {
    if ptr.is_null() { return WrapperResult::null_ptr(); }
    let result = unsafe { &*ptr };
    unsafe {
        *out_ptr = result.clips.as_ptr();
        *out_len = result.clips.len();
    }
    WrapperResult::ok()
}

fn build_layer_rows(
    project: &Project,
    timeline: &Timeline,
    open_ids: &HashSet<u64>,
    parent_abs_frame: i64,
    depth: u32,
    result: &mut Vec<LayerRow>,
) {
    // まず、このtimelineの各レイヤーを1行ずつ作る
    for (layer_order, layer) in timeline.layers.get_sorted_iter().enumerate() {
        let mut clips = Vec::new();
        let mut opened: Vec<(i64, &Timeline)> = Vec::new(); // (abs_frame, 子timeline) を後で展開

        for (_, clip) in &layer.clips {
            let abs_frame = parent_abs_frame + clip.position();
            let is_composite = matches!(clip.clip_data, ClipData::Composite { .. });
            let is_open = is_composite && open_ids.contains(&clip.id);

            clips.push(ClipRenderInfo {
                clip_id: clip.id,
                abs_frame,
                duration: clip.duration,
                is_composite,
                is_open,
            });

            if is_open {
                if let Some(key) = match &clip.clip_data {
                    ClipData::Composite { timeline_id: Some(key) }
                    | ClipData::Area2D { timeline_id: Some(key) }
                    | ClipData::Area3D { timeline_id: Some(key) } => Some(key),
                    _ => None,
                } {
                    if let Some(child_timeline) = project.timelines.get(key) {
                        opened.push((abs_frame, child_timeline));
                    }
                }
            }
        }

        result.push(LayerRow {
            layer_order: layer_order as u32,
            depth,
            clips,
        });

        // 開いてるComposite/Areaの子タイムラインを、このレイヤー行の直後に展開
        for (child_abs_frame, child_timeline) in opened {
            build_layer_rows(
                project,
                child_timeline,
                open_ids,
                child_abs_frame,
                depth + 1,
                result,
            );
        }
    }
}