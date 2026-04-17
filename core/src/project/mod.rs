use std::collections::HashMap;

use esotereel_lib::{
    project::{clip::Clip, timeline::Timeline},
    util::types::ArchivedClipMoveCtx,
};

pub(crate) mod commands;
pub(crate) mod history;

pub(crate) type ClipUpdateMap = Option<HashMap<u32, Vec<Clip>>>;

pub(crate) trait ClipUpdateExt {
    fn push_clip(&mut self, layer_idx: usize, clip: Clip);
}

impl ClipUpdateExt for ClipUpdateMap {
    fn push_clip(&mut self, layer_idx: usize, clip: Clip) {
        if let Some(map) = self {
            map.entry(layer_idx as u32).or_default().push(clip);
        }
    }
}

pub(crate) fn clip_move_mul_core(
    timeline: &mut Timeline,
    moved_clips: &[ArchivedClipMoveCtx],
    updates: &mut ClipUpdateMap,
) {
    let mut extracted: Vec<(usize, usize, i64, i64, Clip)> = Vec::new();

    for ctx in moved_clips {
        let clip_id = ctx.clip_id;
        let new_position = ctx.new_position;
        let new_duration = ctx.new_duration;
        let new_layer = ctx.new_layer as usize;

        let Some((src_layer_idx, _, _)) = timeline.find_clip_by_id(clip_id) else {
            continue;
        };
        let Some(src_layer) = timeline.layers.get_mut(src_layer_idx) else {
            continue;
        };

        let mut extracted_clip: Option<Clip> = None;
        src_layer.clips.retain(|c| {
            if c.id == clip_id && extracted_clip.is_none() {
                extracted_clip = Some(c.clone());
                false
            } else {
                true
            }
        });

        let Some(mut clip) = extracted_clip else {
            continue;
        };

        let original_position = clip.position;
        clip.position = new_position;
        clip.duration = new_duration;
        let delta = new_position - original_position;

        extracted.push((src_layer_idx, new_layer, original_position, delta, clip));
    }

    if extracted.is_empty() {
        return;
    }

    let moving_ids: std::collections::HashSet<u64> =
        extracted.iter().map(|(_, _, _, _, c)| c.id).collect();

    let mut push_left: i64 = 0;
    let mut push_right: i64 = 0;

    for (_, layer_idx, _, _, clip) in &extracted {
        let Some(layer) = timeline.layers.get(*layer_idx) else {
            continue;
        };
        for c in layer.clips.iter() {
            if moving_ids.contains(&c.id) {
                continue;
            }
            if c.position < clip.position + clip.duration && c.position + c.duration > clip.position
            {
                if c.position >= clip.position {
                    let needed = clip.position + clip.duration - c.position;
                    push_left = push_left.max(needed);
                } else {
                    let needed = c.position + c.duration - clip.position;
                    push_right = push_right.max(needed);
                }
            }
        }
    }

    // push_left/push_right を先に確定
    let snap = if push_left > 0 && push_right > 0 {
        None // 挟まれ → キャンセル確定
    } else if push_left > 0 {
        Some(-push_left)
    } else if push_right > 0 {
        Some(push_right)
    } else {
        Some(0)
    };

    // 再衝突チェック
    let cancelled = match snap {
        None => true,
        Some(snap_offset) => extracted.iter().any(|(_, layer_idx, orig, delta, clip)| {
            let final_pos = orig + delta + snap_offset;
            let final_end = final_pos + clip.duration;
            timeline.layers.get(*layer_idx).map_or(false, |layer| {
                layer.clips.iter().any(|c| {
                    !moving_ids.contains(&c.id)
                        && c.position < final_end
                        && c.position + c.duration > final_pos
                })
            })
        }),
    };

    for (src_layer_idx, layer_idx, original_position, delta, mut clip) in extracted {
        if cancelled {
            clip.position = original_position;
            if let Some(layer) = timeline.layers.get_mut(src_layer_idx) {
                layer.clips.insert(clip);
            }
        } else {
            let snap_offset = snap.unwrap_or(0);
            clip.position = original_position + delta + snap_offset;
            updates.push_clip(layer_idx, clip.clone());
            if let Some(layer) = timeline.layers.get_mut(layer_idx) {
                layer.clips.insert(clip);
            }
        }
    }
}

pub(crate) fn clip_add(
    timeline: &mut Timeline,
    layer_idx: usize,
    clip: Clip,
    updates: &mut ClipUpdateMap,
) {
    let Some(layer) = timeline.layers.get_mut(layer_idx) else {
        return;
    };

    // TODO: range check
    updates.push_clip(layer_idx, clip.clone());
    layer.clips.insert(clip);
}
