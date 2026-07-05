use std::sync::Arc;

use esotereel_lib::{
    project::{
        Project, clip_data::ClipData, clip_translate::ClipTranslates, commands::ArchivedCommand,
        layer::Layer, timeline::Timeline,
    },
    util::{
        result::{EsotereelError, EsotereelResult},
        slot_map::SlotMapKey,
    },
};

use rkyv::Deserialize as _;

use crate::project::{ClipUpdateMap, clip_add, clip_move_mul_core};

pub fn handle_command_action(
    request: &ArchivedCommand,
    project: &mut Project,
    timeline_key: &SlotMapKey,
    updates: &mut Option<ClipUpdateMap>,
) -> EsotereelResult<()> {
    let timeline = project
        .timelines
        .get_mut(timeline_key)
        .ok_or(EsotereelError::InvalidTimeline)?;

    match request {
        ArchivedCommand::ClipsMove { clips } => {
            clip_move_mul_core(timeline, clips.as_slice(), updates);
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
            let key: SlotMapKey = layer_map_key.deserialize(&mut rkyv::Infallible).unwrap();

            let clip_data = if let ClipData::Composite { .. } = &clip_data {
                let mut new_timeline = Timeline::new();
                new_timeline
                    .layers
                    .insert(Arc::new(Layer::new(0, "Layer 0".to_string())));

                let new_key = project.timelines.insert(new_timeline);

                ClipData::Composite {
                    timeline_id: Some(new_key),
                }
            } else {
                clip_data
            };

            // ここで改めてtimelineを借用
            let timeline = project
                .timelines
                .get_mut(timeline_key)
                .ok_or(EsotereelError::InvalidTimeline)?;

            let new_clip = timeline
                .layers
                .new_clip(*position, *duration, clip_data, translates);

            clip_add(timeline, &key, new_clip, updates);
        }
    }
    Ok(())
}
