use std::collections::HashMap;

use anyhow::Ok;
use esotereel_lib::{
    plugin::{NamespacedID, property::value::FieldValue},
    project::{
        Clip, Project, TimelineTick,
        clip::ClipBindingValue,
        command::{ClipMoveHistoryCtx, ClipResizeHistoryCtx},
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
    let extracted = {
        let timeline = project
            .timeline_mut(timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

        // (src_layer_id, dest_layer_id, orig_pos, orig_dur, new_pos, new_dur, clip)
        let mut extracted: Vec<(LayerId, LayerId, i64, i64, i64, i64, Clip)> = Vec::new();

        // クリップの取り出し処理
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
        extracted
    };

    // 重なり判定
    let timeline_ref = project
        .timeline_ref(timeline_id)
        .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

    let exclude_ids: Vec<_> = moved_clips.iter().map(|ctx| ctx.clip_id).collect();
    let has_overlap = extracted
        .iter()
        .any(|(_, dest_layer_id, _, _, new_pos, new_dur, _)| {
            !timeline_ref.can_place_clip_at(*dest_layer_id, *new_pos, *new_dur, &exclude_ids)
        });

    let timeline = project
        .timeline_mut(timeline_id)
        .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

    if has_overlap {
        // 重なりがある場合は元に戻す
        for (src_layer_id, _dest_layer_id, orig_pos, orig_dur, _new_pos, _new_dur, mut clip) in
            extracted
        {
            clip.set_position(orig_pos);
            clip.duration = orig_dur;
            let _ = timeline.place_clip(src_layer_id, clip);
        }
        return Ok(());
    }

    // 2. 移動先に配置
    for (src_layer_id, dest_layer_id, _orig_pos, _orig_dur, new_pos, new_dur, mut clip) in extracted
    {
        clip.set_position(new_pos);
        clip.duration = new_dur;
        let _ = timeline.place_clip(dest_layer_id, clip);
    }

    Ok(())
}

pub(crate) fn clip_resize_core(
    project: &mut Project,
    timeline_id: TimelineId,
    resized_clips: &[ClipResizeHistoryCtx],
) -> anyhow::Result<()> {
    // 重なり判定
    {
        let timeline = project
            .timeline_ref(timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
        let exclude_ids: Vec<_> = resized_clips.iter().map(|c| c.clip_id).collect();
        for ctx in resized_clips {
            let (_, layer_id) = timeline
                .get_clip_and_layer(ctx.clip_id)
                .ok_or(EsotereelError::ClipNotFound(ctx.clip_id))?;
            if !timeline.can_place_clip_at(
                layer_id,
                ctx.new_position,
                ctx.new_duration,
                &exclude_ids,
            ) {
                return Ok(());
            }
        }
    }

    let timeline = project
        .timeline_mut(timeline_id)
        .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

    // クリップの取り出し処理
    let mut extracted = Vec::new();
    for ctx in resized_clips {
        let (clip, layer_id) = timeline
            .remove_clip_by_id(ctx.clip_id)
            .ok_or(EsotereelError::ClipNotFound(ctx.clip_id))?;
        extracted.push((layer_id, ctx, clip));
    }

    // 新しい位置・長さで同じレイヤーに置き直す
    for (layer_id, ctx, mut clip) in extracted {
        clip.set_position(ctx.new_position);
        clip.duration = ctx.new_duration;
        timeline.place_clip(layer_id, clip)?;
    }

    // source_offset (placeholder)
    // TODO: いろいろ検討
    let key = NamespacedID::new("std", "source_offset").unwrap();
    for ctx in resized_clips {
        if let Some(off) = ctx.new_source_offset {
            let v = ClipBindingValue::Static(FieldValue::Int(off));
            timeline
                .get_clip_mut(ctx.clip_id)
                .ok_or(EsotereelError::ClipNotFound(ctx.clip_id))?
                .set_property_value(key.clone(), v.clone())?;
            timeline.touch_property_changed(ctx.clip_id, key.clone(), v);
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
    if let Some(existing) = preferred_clip_id {
        project.id_generator_mut().observe_clip(existing);
        let timeline = project
            .timeline_mut(timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
        let clip = Clip::new(
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
