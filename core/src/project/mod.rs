use anyhow::Ok;
use esotereel_lib::{
    project::{
        Clip, Project, TimelineTick,
        clip::{ClipData, CompositionRef},
        ids::{LayerId, TimelineId},
        transform::ClipTranslates,
    },
    util::{result::EsotereelError, types::ArchivedClipMoveCtx},
};

pub(crate) mod commands;
pub(crate) mod history;

pub(crate) fn clip_move_mul_core(
    project: &mut Project,
    timeline_id: TimelineId,
    moved_clips: &[ArchivedClipMoveCtx],
) -> anyhow::Result<()> {
    let timeline = project
        .timeline_mut(timeline_id)
        .ok_or(EsotereelError::InvalidTimeline)?;

    // (src_layer_id, dest_layer_id, orig_pos, orig_dur, new_pos, clip)
    let mut extracted: Vec<(LayerId, LayerId, i64, i64, i64, Clip)> = Vec::new();

    // 1. クリップの取り出し処理
    // remove_clip_by_id が所属レイヤーの検索・除去・Clip実体の取り出しを一括でやってくれる
    for ctx in moved_clips {
        if let Some((clip, src_layer_id)) = timeline.remove_clip_by_id(ctx.clip_id) {
            let original_position = clip.position();
            let original_duration = clip.duration;

            let dest_layer_id = ctx.new_layer_id;
            let new_position = ctx.new_position;

            extracted.push((
                src_layer_id,
                dest_layer_id,
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
    for (src_layer_id, dest_layer_id, orig_pos, orig_dur, new_pos, mut clip) in extracted {
        if cancelled {
            // キャンセル時は元のレイヤー・位置に戻す
            clip.set_position(orig_pos);
            clip.duration = orig_dur;
            let _ = timeline.place_clip(src_layer_id, clip);
        } else {
            // 確定時は移動先のレイヤーに配置
            clip.set_position(new_pos);

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
    clip_data: ClipData,
    translates: ClipTranslates,
) -> anyhow::Result<()> {
    let clip_data = if let ClipData::Composite { .. } = &clip_data {
        // 新しいタイムラインを作成 (Project::new_timeline を使用)
        let new_timeline_id = project.insert_timeline(60.0);

        ClipData::Composite {
            source: CompositionRef::Independent(new_timeline_id),
        }
    } else {
        clip_data
    };

    // key (u32) をそのまま渡してクリップを追加
    project.new_clip_in_timeline(
        timeline_id,
        layer_id,
        position,
        duration,
        clip_data,
        translates,
    )?;

    Ok(())
}
