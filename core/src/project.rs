use std::collections::HashMap;

use nomyoedit_lib::{
    project::{clip::Clip, timeline::Timeline},
    types::ClipMoveCtx,
};

pub(crate) fn clip_move_mul_core(
    timeline: &mut Timeline,
    moved_clips: Vec<ClipMoveCtx>,
    updates: &mut HashMap<usize, Vec<Clip>>,
) {
    let update_ids: Vec<u64> = moved_clips.iter().map(|c| c.clip_id).collect();
    for clip_ctx in &moved_clips {
        if let Some((_, _, c)) = timeline.find_clip_by_id(clip_ctx.clip_id) {
            if timeline.would_clip_overlap(
                clip_ctx.new_layer as usize,
                clip_ctx.new_frame,
                c.duration,
                &update_ids,
            ) {
                return; // overraped
            }
        }
    }

    let mut extracted_clips = Vec::new();

    // 1. 位置の検索と「取り出し」を行う
    for clip_ctx in &moved_clips {
        // IDから「どのレイヤーにあるか」と「Clipの実体」を特定
        // timeline 自体を不変借用し続けるのを防ぐため、必要な情報だけ先に取得
        if let Some((layer_idx, _, c)) = timeline.find_clip_by_id(clip_ctx.clip_id) {
            let dummy = c.clone();
            // レイヤーを取得
            if let Some(layer) = timeline.layers.get_mut(layer_idx) {
                // 検索用のダミー（IDが比較対象ならIDだけ合致させればよい）
                if let Some(mut clip) = layer.clips.take(&dummy) {
                    // 新しい座標を計算（所有権を持っているので自由に書き換え可能）
                    clip.position = clip_ctx.new_frame as u64;
                    let target_layer_idx = clip_ctx.new_layer as usize;

                    extracted_clips.push((target_layer_idx, clip));
                }
            }
        }
    }

    // 2. すべて取り出し終わった後に「挿入」する
    for (target_layer_idx, clip) in extracted_clips {
        // ターゲットのレイヤーが存在するか確認し、なければ適宜作成するか無視する
        let layer = timeline
            .layers
            .get_mut(target_layer_idx)
            .expect("layer range over! this never happen becauce checked before");

        updates
            .entry(target_layer_idx)
            .or_default()
            .push(clip.clone());

        layer.clips.insert(clip);
    }
}
