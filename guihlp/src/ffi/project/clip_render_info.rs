use std::collections::HashSet;

use esotereel_lib::project::{
    Project, Timeline,
    clip::ClipData,
    ids::{LayerFolderId, LayerId, TimelineId},
    layer_outline::OutlineNode,
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

pub enum LayerRowKind {
    Layer(LayerId),
    Folder(LayerFolderId),
}

pub struct LayerRow {
    pub kind: LayerRowKind,
    pub timeline_id: TimelineId, // どのTimelineに属する行か(Composite展開行対応)
    pub depth: u32,
    pub is_folder_open: bool,
    pub clips: Vec<ClipRenderInfo>,
}

#[repr(C)]
pub enum FfiLayerRowKind {
    Layer,
    Folder,
}

#[repr(C)]
pub struct FfiLayerRow {
    pub node_kind: FfiLayerRowKind,
    pub node_id: LayerId,
    pub timeline_id: TimelineId,
    pub depth: u32,
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
        &timeline.outline.roots,
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
        let (node_kind, node_id) = match row.kind {
            LayerRowKind::Layer(id) => (FfiLayerRowKind::Layer, id),
            LayerRowKind::Folder(id) => (FfiLayerRowKind::Folder, id),
        };
        rows.push(FfiLayerRow {
            node_kind,
            node_id,
            timeline_id: row.timeline_id,
            depth: row.depth,
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

fn build_layer_rows(
    project: &Project,
    timeline: &Timeline,
    nodes: &[OutlineNode],
    open_ids: &HashSet<u64>,
    open_folder_ids: &HashSet<u64>,
    parent_abs_frame: i64,
    depth: u32,
    result: &mut Vec<LayerRow>,
) {
    for node in nodes {
        build_layer_row_recursive(
            project,
            timeline,
            node,
            open_ids,
            open_folder_ids,
            parent_abs_frame,
            depth,
            result,
        );
    }
}

fn build_layer_row_recursive<'a>(
    project: &'a Project,
    timeline: &'a Timeline,
    node: &OutlineNode,
    open_ids: &HashSet<u64>,
    open_folder_ids: &HashSet<u64>,
    parent_abs_frame: i64,
    depth: u32,
    result: &mut Vec<LayerRow>,
) {
    match *node {
        OutlineNode::Layer(layer_id) => {
            let Some(layer) = timeline.get_layer(layer_id) else {
                return;
            };
            let mut clips = Vec::new();
            let mut opened: Vec<(i64, &'a Timeline)> = Vec::new();

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
                kind: LayerRowKind::Layer(layer_id),
                timeline_id: timeline.id,
                depth,
                is_folder_open: false,
                clips,
            });

            for (child_abs_frame, child_timeline) in opened {
                build_layer_rows(
                    project,
                    child_timeline,
                    &child_timeline.outline.roots,
                    open_ids,
                    open_folder_ids,
                    child_abs_frame,
                    depth + 1,
                    result,
                );
            }
        }
        OutlineNode::Folder(folder_id) => {
            let Some(folder) = timeline.outline.get_folder(folder_id) else {
                return;
            };
            let is_open = open_folder_ids.contains(&folder_id);

            result.push(LayerRow {
                kind: LayerRowKind::Folder(folder_id),
                timeline_id: timeline.id,
                depth,
                is_folder_open: is_open,
                clips: Vec::new(),
            });

            if is_open {
                build_layer_rows(
                    project,
                    timeline,
                    &folder.children,
                    open_ids,
                    open_folder_ids,
                    parent_abs_frame,
                    depth + 1,
                    result,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esotereel_lib::project::Project;

    #[test]
    fn build_layer_rows_preserves_insertion_order() {
        let mut project = Project::new();
        let timeline_id = project.insert_timeline(60.0);

        let five = project
            .insert_layer_in_timeline(timeline_id, None, None, "five".into())
            .unwrap();
        let one = project
            .insert_layer_in_timeline(timeline_id, None, None, "one".into())
            .unwrap();
        let three = project
            .insert_layer_in_timeline(timeline_id, None, None, "three".into())
            .unwrap();

        let mut rows = Vec::new();
        {
            let timeline = project.timeline(timeline_id).unwrap();
            build_layer_rows(
                &project,
                timeline,
                &timeline.outline.roots,
                &HashSet::new(),
                &HashSet::new(),
                0,
                0,
                &mut rows,
            );
        }

        let ids: Vec<u64> = rows
            .iter()
            .filter_map(|row| match row.kind {
                LayerRowKind::Layer(id) => Some(id),
                LayerRowKind::Folder(_) => None,
            })
            .collect();

        // デフォルト4レイヤーの後ろに、insert順のままfive, one, threeが続く
        assert_eq!(&ids[ids.len() - 3..], &[five, one, three]);
    }
}
