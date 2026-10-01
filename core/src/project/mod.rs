use std::collections::{BTreeMap, HashMap};

use anyhow::Ok;
use esotereel_lib::{
    plugin::NamespacedID,
    project::{
        Clip, Project, TimelineTick,
        clip::ClipBindingValue,
        command::ClipMoveHistoryCtx,
        ids::{ClipId, LayerId, TimelineId},
        transform::ClipTranslates,
    },
    util::result::EsotereelError,
};

pub(crate) mod commands;
pub(crate) mod history;

pub(crate) fn clip_move_mul_core(
    project: &mut Project,
    timeline_id: TimelineId,
    moved_clips: &[ClipMoveHistoryCtx],
) -> anyhow::Result<()> {
    let timeline = project
        .timeline_mut(timeline_id)
        .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

    // (src_layer_id, dest_layer_id, orig_pos, orig_dur, new_pos, new_dur, clip)
    let mut extracted: Vec<(LayerId, LayerId, i64, i64, i64, i64, Clip)> = Vec::new();

    // 1. クリップの取り出し処理
    // remove_clip_by_id が所属レイヤーの検索・除去・Clip実体の取り出しを一括でやってくれる
    for ctx in moved_clips {
        if let Some((clip, src_layer_id)) = timeline.remove_clip_by_id(ctx.clip_id) {
            let original_position = clip.position();
            let original_duration = clip.duration;

            let dest_layer_id = ctx.new_layer_id;
            let new_position = ctx.new_position;
            let new_duration = ctx.new_duration;

            extracted.push((
                src_layer_id,
                dest_layer_id,
                original_position,
                original_duration,
                new_position,
                new_duration,
                clip,
            ));
        }
    }

    // 重なり判定・キャンセルなどのフラグ（必要に応じて調整）
    let cancelled = false;

    // 2. 移動先または元に戻す処理
    for (src_layer_id, dest_layer_id, orig_pos, orig_dur, new_pos, new_dur, mut clip) in extracted {
        if cancelled {
            // キャンセル時は元のレイヤー・位置に戻す
            clip.set_position(orig_pos);
            clip.duration = orig_dur;
            let _ = timeline.place_clip(src_layer_id, clip);
        } else {
            // 確定時は移動先のレイヤーに配置
            clip.set_position(new_pos);
            clip.duration = new_dur;

            let _ = timeline.place_clip(dest_layer_id, clip);
        }
    }

    Ok(())
}

pub(crate) fn clip_add_core(
    project: &mut Project,
    timeline_id: TimelineId,
    layer_id: LayerId,
    position: TimelineTick,
    duration: TimelineTick,
    kind_id: NamespacedID,
    properties: HashMap<NamespacedID, ClipBindingValue>,
    translates: ClipTranslates,
    preferred_clip_id: Option<ClipId>,
) -> anyhow::Result<ClipId> {
    let timeline = project
        .timeline_mut(timeline_id)
        .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

    let clip_id = preferred_clip_id.unwrap_or_else(|| {
        let id = project.id_generator_mut().next_clip_id();
        id
    });

    if let Some(existing) = preferred_clip_id {
        project.id_generator_mut().observe_clip(existing);
        let clip = crate::project::clip::Clip::new(
            existing, position, duration, kind_id, properties, translates,
        );
        timeline.place_clip(layer_id, clip)?;
        return Ok(existing);
    }

    let clip_id = project.new_clip_in_timeline(
        timeline_id,
        layer_id,
        position,
        duration,
        kind_id,
        properties,
        translates,
    )?;

    Ok(clip_id)
}
