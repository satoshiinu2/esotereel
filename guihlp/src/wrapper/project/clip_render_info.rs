use std::collections::HashSet;

use esotereel_lib::project::{Project, Timeline, clip::ClipData};

use crate::{WrapperErrorCode, slice_from_ptr_safe};

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
) -> WrapperErrorCode {
    if project.is_null() || timeline.is_null() || out.is_null() {
        return WrapperErrorCode::NullPtr;
    }
    let project = unsafe { &*project };
    let timeline = unsafe { &*timeline };
    let open_ids: HashSet<u64> = slice_from_ptr_safe(open_ids_ptr, open_ids_len)
        .iter()
        .cloned()
        .collect();

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

    use esotereel_lib::project::{Layer, Project, Timeline};

    use super::build_layer_rows;

    #[test]
    fn build_layer_rows_follows_layer_order() {
        let mut project = Project::new();
        let timeline_id = project.insert_timeline(60.0);

        {
            let timeline = project.timeline_mut(timeline_id).unwrap();
            timeline
                .insert_layer(Layer::new(100, 5, "five".into()))
                .unwrap();
            timeline
                .insert_layer(Layer::new(200, 1, "one".into()))
                .unwrap();
            timeline
                .insert_layer(Layer::new(300, 3, "three".into()))
                .unwrap();
        }

        let mut rows = Vec::new();
        {
            let timeline = project.timeline(timeline_id).unwrap();
            build_layer_rows(&project, timeline, &HashSet::new(), 0, 0, &mut rows);
        }

        let orders: Vec<u32> = rows.iter().map(|row| row.layer_order).collect();
        assert_eq!(orders, vec![1, 3, 5]);
    }
}

fn build_layer_rows(
    project: &Project,
    timeline: &Timeline,
    open_ids: &HashSet<u64>,
    parent_abs_frame: i64,
    depth: u32,
    result: &mut Vec<LayerRow>,
) {
    // HashMap の反復順ではレイヤー表示順が崩れるので、order順で排出する
    for layer_order in timeline.iter_sorted().map(|layer| layer.order) {
        let Some(layer) = timeline
            .get_layer_id_by_order(layer_order)
            .and_then(|layer_id| timeline.get_layer(layer_id))
        else {
            continue;
        };

        let mut clips = Vec::new();
        let mut opened: Vec<(i64, &Timeline)> = Vec::new(); // (abs_frame, 子timeline) を後で展開

        for clip in layer.clips.iter() {
            let abs_frame = parent_abs_frame + clip.position;
            let is_composite = matches!(clip.data, ClipData::Composite { .. });
            let is_open = is_composite && open_ids.contains(&clip.id);

            clips.push(ClipRenderInfo {
                clip_id: clip.id,
                abs_frame,
                duration: clip.duration,
                is_composite,
                is_open,
            });

            if is_open {
                if let Some(key) = match clip.data {
                    ClipData::Composite {
                        timeline_id: Some(key),
                    }
                    | ClipData::Area2D {
                        timeline_id: Some(key),
                    }
                    | ClipData::Area3D {
                        timeline_id: Some(key),
                    } => Some(key),
                    _ => None,
                } {
                    if let Some(child_timeline) = project.timeline(key) {
                        opened.push((abs_frame, child_timeline));
                    }
                }
            }
        }

        result.push(LayerRow {
            layer_order,
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
