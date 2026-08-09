use std::sync::Arc;

use esotereel_lib::{
    project::{Clip, ClipUpdateMap, LayerMapKey, Timeline},
    util::types::ArchivedClipMoveCtx,
};

pub(crate) mod commands;
pub(crate) mod history;

pub(crate) trait ClipUpdateExt {
    fn push_clip(&mut self, layer_order: LayerMapKey, clip: Clip);
}

impl ClipUpdateExt for Option<ClipUpdateMap> {
    fn push_clip(&mut self, order: LayerMapKey, clip: Clip) {
        if let Some(map) = self {
            map.entry(order as _).or_default().push(clip);
        }
    }
}

pub(crate) fn clip_move_mul_core(
    timeline: &mut Timeline,
    moved_clips: &[ArchivedClipMoveCtx],
    updates: &mut Option<ClipUpdateMap>,
) {
    // (src_layer_order, dest_layer_order, orig_pos, orig_dur, new_pos, clip)
    let mut extracted: Vec<(LayerMapKey, LayerMapKey, i64, i64, i64, Clip)> = Vec::new();

    // 1. クリップの取り出し処理
    for ctx in moved_clips {
        let mut found = None;

        // すべてのレイヤーから該当する clip_id を探して削除・抽出
        for (&layer_order, layer) in timeline.iter_layers_mut() {
            if let Some(clip) = layer.clips.remove_by_id(ctx.clip_id) {
                found = Some((layer_order, clip));
                break;
            }
        }

        if let Some((src_layer_order, clip)) = found {
            let original_position = clip.position();
            let original_duration = clip.duration;

            // new_layer_map_key と new_position を使用
            let dest_layer_order = ctx.new_layer_id;
            let new_position = ctx.new_position;

            extracted.push((
                src_layer_order,
                dest_layer_order,
                original_position,
                original_duration,
                new_position,
                clip,
            ));
        }
    }

    // 重なり判定・キャンセルなどのフラグ（必要に応じて調整）
    let cancelled = false;

    // 2. 移動先または元に戻す処理
    for (src_layer_order, dest_layer_order, orig_pos, orig_dur, new_pos, mut clip) in extracted {
        if cancelled {
            // キャンセル時は元のレイヤーに戻す
            if let Some(layer) = timeline.get_layer_mut(src_layer_order) {
                clip.set_position(orig_pos);
                clip.duration = orig_dur;
                let _ = layer.clips.insert(clip);
            }
        } else {
            // 確定時は移動先のレイヤーに挿入
            if let Some(layer) = timeline.get_layer_mut(dest_layer_order) {
                clip.set_position(new_pos);

                // クライアント通知用
                updates.push_clip(dest_layer_order, clip.clone());

                let _ = layer.clips.insert(clip);
            }
        }
    }
}

pub(crate) fn clip_add(
    timeline: &mut Timeline,
    layer_order: LayerMapKey,
    clip: Clip,
    updates: &mut Option<ClipUpdateMap>,
) {
    if let Some(layer) = timeline.get_layer_mut(layer_order) {
        updates.push_clip(layer_order, clip.clone());
        let _ = layer.clips.insert(clip);
    }
}
