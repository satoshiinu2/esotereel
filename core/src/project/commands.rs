use esotereel_lib::{
    project::{
        Clip, ClipUpdateMap, Project, clip::ClipData, commands::ArchivedCommand, ids::TimelineId,
        transform::ClipTranslates,
    },
    util::result::{EsotereelError, EsotereelResult},
};

use rkyv::Deserialize as _;

use crate::project::{ClipUpdateExt, clip_move_mul_core};

pub fn handle_command_action(
    request: &ArchivedCommand,
    project: &mut Project,
    timeline_key: TimelineId,
    updates_map: &mut Option<ClipUpdateMap>,
) -> EsotereelResult<()> {
    match request {
        ArchivedCommand::ClipsMove { clips } => {
            let timeline = project
                .timeline_mut(timeline_key)
                .ok_or(EsotereelError::InvalidTimeline)?;
            clip_move_mul_core(timeline, clips.as_slice(), updates_map);
        }
        ArchivedCommand::AddClip {
            layer_map_key,
            position,
            duration,
            clip_data,
            translates,
        } => {
            let clip_data: ClipData = clip_data.deserialize(&mut rkyv::Infallible).unwrap();
            let translates: ClipTranslates = translates.deserialize(&mut rkyv::Infallible).unwrap();

            let clip_data = if let ClipData::Composite { .. } = &clip_data {
                // 新しいタイムラインを作成 (Project::new_timeline を使用)
                let new_timeline_id = project.insert_timeline(60.0);

                ClipData::Composite {
                    timeline_id: Some(new_timeline_id),
                }
            } else {
                clip_data
            };

            let id_generator = project.id_generator_mut();
            let timeline = project
                .timeline_mut(timeline_key)
                .ok_or(EsotereelError::InvalidTimeline)?;

            // key (u32) をそのまま渡してクリップを追加
            let new_clip_id = project.new_clip_in_timeline(
                timeline_key,
                *layer_map_key,
                *position,
                *duration,
                clip_data,
                translates,
                Some(|c: &Clip| updates_map.push_clip(*layer_map_key, c.clone())),
            )?;
        }
    }
    Ok(())
}
