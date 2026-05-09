use std::sync::Arc;

use esotereel_lib::{
    project::{ClipUpdateMap, clip::Clip, timeline::Timeline},
    util::types::ArchivedClipMoveCtx,
};

pub(crate) mod commands;
pub(crate) mod history;

pub(crate) trait ClipUpdateExt {
    fn push_clip(&mut self, layer_handle: u32, clip: Arc<Clip>);
}

impl ClipUpdateExt for Option<ClipUpdateMap> {
    fn push_clip(&mut self, layer_handle: u32, clip: Arc<Clip>) {
        if let Some(map) = self {
            map.entry(layer_handle).or_default().push(clip);
        }
    }
}

pub(crate) fn clip_move_mul_core(
    timeline: &mut Timeline,
    moved_clips: &[ArchivedClipMoveCtx],
    updates: &mut Option<ClipUpdateMap>,
) {
    // src_layer_handle, dest_layer_handle, orig_pos, orig_dur, delta, clip
    let mut extracted: Vec<(u32, u32, i64, i64, i64, Arc<Clip>)> = Vec::new();

    // まず対象のクリップをタイムラインから取り出し、情報をまとめる
    for ctx in moved_clips {
        if let Some((_, clip, src_layer_handle)) = timeline.remove_clip_by_id(ctx.clip_id) {
            let mut clip_to_move = clip;
            let original_position = clip_to_move.position();
            let original_duration = clip_to_move.duration;

            let clip_ref = Arc::make_mut(&mut clip_to_move);

            // クリップ自体のデータを更新 (衝突判定や最終的な挿入で使用される)
            clip_ref.set_position(ctx.new_position);
            clip_ref.duration = ctx.new_duration;

            let delta = ctx.new_position - original_position;
            extracted.push((
                src_layer_handle,
                ctx.new_layer_handle,
                original_position,
                original_duration,
                delta,
                clip_to_move,
            ));
        }
    }

    if extracted.is_empty() {
        return;
    }

    // キャッシュ
    let moving_ids: std::collections::HashSet<u64> =
        extracted.iter().map(|(_, _, _, _, _, c)| c.id).collect();

    let mut snap_left: i64 = 0;
    let mut snap_right: i64 = 0;

    // 衝突判定 (新しい位置での重なりをチェック)
    for (_, layer_idx, _, _, _, clip) in &extracted {
        let Some(layer) = timeline.layers.get_by_layer_handle(*layer_idx) else {
            continue;
        };

        // 検索範囲
        let start_search = clip.position();
        let end_search = clip.position() + clip.duration;

        let prev_clip = layer.clips.range(..=start_search).next_back();

        let in_range = layer.clips.range(start_search..=end_search);

        for (_, c) in prev_clip.into_iter().chain(in_range) {
            if moving_ids.contains(&c.id) {
                continue;
            }

            // 衝突判定
            if c.position() < clip.position() + clip.duration
                && c.position() + c.duration > clip.position()
            {
                if c.position() >= clip.position() {
                    snap_left = snap_left.max(clip.position() + clip.duration - c.position());
                } else {
                    snap_right = snap_right.max(c.position() + c.duration - clip.position());
                }
            }
        }
    }

    // 最終的なスナップオフセットを決定
    let final_snap = if snap_left > 0 && snap_right > 0 {
        None // 挟まれ → キャンセル確定
    } else if snap_left > 0 {
        Some(-snap_left)
    } else if snap_right > 0 {
        Some(snap_right)
    } else {
        Some(0)
    };

    // 再衝突チェック
    let cancelled = match final_snap {
        None => true,
        Some(snap_offset) => extracted
            .iter()
            .any(|(_, layer_idx, orig, _, delta, clip)| {
                let final_pos = orig + delta + snap_offset;
                let final_end = final_pos + clip.duration;
                timeline
                    .layers
                    .get_by_layer_handle(*layer_idx)
                    .map_or(false, |layer| {
                        layer.clips.into_iter().any(|(_, c)| {
                            !moving_ids.contains(&c.id)
                                && c.position() < final_end
                                && c.position() + c.duration > final_pos
                        })
                    })
            }),
    };

    for (src_layer_idx, dest_layer_idx, orig_pos, orig_dur, delta, mut clip) in extracted {
        if cancelled {
            // キャンセルなら元の位置に戻す
            if let Some(layer) = timeline.layers.get_by_layer_handle_mut(src_layer_idx) {
                let layer = Arc::make_mut(layer);
                let c = Arc::make_mut(&mut clip);
                c.set_position(orig_pos);
                c.duration = orig_dur;

                layer.clips.insert(clip);
            }
        } else {
            // 確定なら移動先に挿入
            if let Some(layer) = timeline.layers.get_by_layer_handle_mut(dest_layer_idx) {
                let snap_offset = final_snap.unwrap_or(0);
                let new_pos = orig_pos + delta + snap_offset;

                let layer = Arc::make_mut(layer);
                Arc::make_mut(&mut clip).set_position(new_pos);

                // クライアント通知用
                updates.push_clip(dest_layer_idx, clip.clone());

                layer.clips.insert(clip);
            }
        }
    }
}

pub(crate) fn clip_add(
    timeline: &mut Timeline,
    layer_handle: u32,
    clip: Arc<Clip>,
    updates: &mut Option<ClipUpdateMap>,
) {
    let Some(layer) = timeline.layers.get_by_layer_handle_mut(layer_handle) else {
        return;
    };

    // TODO: range check
    updates.push_clip(layer_handle, clip.clone());
    Arc::make_mut(layer).clips.insert(clip);
}
