use std::collections::HashSet;

use esotereel_lib::project::{
    Project, Timeline,
    clip::ClipData,
    ids::{LayerId, TimelineId},
};

use crate::{WrapperErrorCode, slice_from_ptr_or_empty};

#[repr(C)]
pub struct ClipRenderInfo {
    pub clip_id: u64,
    pub abs_frame: i64,
    pub duration: i64,
    pub is_composite: bool,
    pub is_open: bool,
}

pub struct LayerRow {
    pub layer_id: LayerId,
    pub timeline_id: TimelineId, // どのTimelineに属する行か(Composite展開行対応)
    pub depth: u32,
    pub is_folder: bool,
    pub is_folder_open: bool,
    pub clips: Vec<ClipRenderInfo>,
}

#[repr(C)]
pub struct FfiLayerRow {
    pub layer_id: LayerId,
    pub timeline_id: TimelineId,
    pub depth: u32,
    pub is_folder: bool,
    pub is_folder_open: bool,
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
    open_folder_ids_ptr: *const u64,
    open_folder_ids_len: usize,
    out: *mut *mut RenderRowsResult,
) -> WrapperErrorCode {
    if project.is_null() || timeline.is_null() || out.is_null() {
        return WrapperErrorCode::NullPtr;
    }
    let project = unsafe { &*project };
    let timeline = unsafe { &*timeline };
    let open_ids: HashSet<u64> = unsafe {
        slice_from_ptr_or_empty(open_ids_ptr, open_ids_len)
            .iter()
            .cloned()
            .collect()
    };
    let open_folder_ids: HashSet<u64> = unsafe {
        slice_from_ptr_or_empty(open_folder_ids_ptr, open_folder_ids_len)
            .iter()
            .cloned()
            .collect()
    };

    let mut layer_rows = Vec::new();
    build_layer_rows(
        project,
        timeline,
        &open_ids,
        &open_folder_ids,
        0,
        0,
        &mut layer_rows,
    );

    // フラット化: rows + clips の2本の配列に変換
    let mut rows = Vec::with_capacity(layer_rows.len());
    let mut clips = Vec::new();
    for row in layer_rows {
        let clip_start = clips.len() as u32;
        let clip_count = row.clips.len() as u32;
        clips.extend(row.clips);
        rows.push(FfiLayerRow {
            layer_id: row.layer_id,
            timeline_id: row.timeline_id,
            depth: row.depth,
            is_folder: row.is_folder,
            is_folder_open: row.is_folder_open,
            clip_start,
            clip_count,
        });
    }

    unsafe { *out = Box::into_raw(Box::new(RenderRowsResult { rows, clips })) };
    WrapperErrorCode::Ok
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
) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let result = unsafe { &*ptr };
    unsafe {
        *out_ptr = result.rows.as_ptr();
        *out_len = result.rows.len();
    }
    WrapperErrorCode::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_rows_get_clips(
    ptr: *const RenderRowsResult,
    out_ptr: *mut *const ClipRenderInfo,
    out_len: *mut usize,
) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let result = unsafe { &*ptr };
    unsafe {
        *out_ptr = result.clips.as_ptr();
        *out_len = result.clips.len();
    }
    WrapperErrorCode::Ok
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use esotereel_lib::project::{Layer, Project};

    use super::build_layer_rows;

    #[test]
    fn build_layer_rows_preserves_insertion_order() {
        let mut project = Project::new();
        let timeline_id = project.insert_timeline(60.0);

        {
            let timeline = project.timeline_mut(timeline_id).unwrap();
            timeline
                .insert_layer(Layer::new(100, "five".into()), None, None)
                .unwrap();
            timeline
                .insert_layer(Layer::new(200, "one".into()), None, None)
                .unwrap();
            timeline
                .insert_layer(Layer::new(300, "three".into()), None, None)
                .unwrap();
        }

        let mut rows = Vec::new();
        {
            let timeline = project.timeline(timeline_id).unwrap();
            build_layer_rows(
                &project,
                timeline,
                &HashSet::new(),
                &HashSet::new(),
                0,
                0,
                &mut rows,
            );
        }

        // orderという数値は無くなったので、並びはroot_layersへのinsert順(Vec)そのもの
        let ids: Vec<u64> = rows.iter().map(|row| row.layer_id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 100, 200, 300]);
    }
}

fn build_layer_rows(
    project: &Project,
    timeline: &Timeline,
    open_ids: &HashSet<u64>,
    open_folder_ids: &HashSet<u64>,
    parent_abs_frame: i64,
    depth: u32,
    result: &mut Vec<LayerRow>,
) {
    // 旧: order順に排出 → 新: root_layers(Vec)の並びがそのまま表示順
    for &layer_id in timeline.root_layers() {
        build_layer_row_recursive(
            project,
            timeline,
            layer_id,
            open_ids,
            open_folder_ids,
            parent_abs_frame,
            depth,
            result,
        );
    }
}

/// 1レイヤー行(と、開いていればその子行/子Timeline)を再帰的にresultへ積む。
/// Folder(children持ち)は自分自身も1行として出しつつ、開いていれば直後に
/// children を depth+1 で展開する。Composite/Areaクリップの子Timeline展開と
/// 独立して制御できるよう、open_ids(Composite用)とopen_folder_ids(Folder用)を分けている。
fn build_layer_row_recursive<'a>(
    project: &'a Project,
    timeline: &'a Timeline,
    layer_id: LayerId,
    open_ids: &HashSet<u64>,
    open_folder_ids: &HashSet<u64>,
    parent_abs_frame: i64,
    depth: u32,
    result: &mut Vec<LayerRow>,
) {
    let Some(layer) = timeline.get_layer(layer_id) else {
        return;
    };

    let is_folder = layer.is_folder();
    let is_folder_open = is_folder && open_folder_ids.contains(&layer_id);

    let mut clips = Vec::new();
    let mut opened: Vec<(i64, &'a Timeline)> = Vec::new(); // (abs_frame, 子timeline) を後で展開

    for (&pos, &clip_id) in &layer.clips {
        let Some(clip) = timeline.get_clip(clip_id) else {
            continue;
        };
        let abs_frame = parent_abs_frame + pos;
        let is_composite = matches!(
            clip.data,
            ClipData::Composite { .. } | ClipData::Area2D { .. } | ClipData::Area3D { .. }
        );
        let is_open = is_composite && open_ids.contains(&clip.id);

        clips.push(ClipRenderInfo {
            clip_id: clip.id,
            abs_frame,
            duration: clip.duration,
            is_composite,
            is_open,
        });

        if is_open {
            if let Some(child_id) = clip.data.nested_timeline_id() {
                if let Some(child_timeline) = project.timeline(child_id) {
                    opened.push((abs_frame, child_timeline));
                }
            }
        }
    }

    result.push(LayerRow {
        layer_id,
        timeline_id: timeline.id,
        depth,
        is_folder,
        is_folder_open,
        clips,
    });

    // フォルダーが開いていれば、children を直後に depth+1 で展開
    if is_folder_open {
        for &child_layer_id in &layer.children {
            build_layer_row_recursive(
                project,
                timeline,
                child_layer_id,
                open_ids,
                open_folder_ids,
                parent_abs_frame,
                depth + 1,
                result,
            );
        }
    }

    // 開いてるComposite/Areaの子タイムラインを、このレイヤー行の直後に展開
    for (child_abs_frame, child_timeline) in opened {
        build_layer_rows(
            project,
            child_timeline,
            open_ids,
            open_folder_ids,
            child_abs_frame,
            depth + 1,
            result,
        );
    }
}
