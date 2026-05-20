use esotereel_lib::{
    project::{
        clip_data::ClipData, clip_translate::ClipTranslates, commands::ArchivedCommand,
        timeline::Timeline,
    },
    util::{result::EsotereelResult, slot_map::SlotMapKey},
};

use rkyv::Deserialize as _;

use crate::project::{ClipUpdateMap, clip_add, clip_move_mul_core};

pub fn handle_command_action(
    request: &ArchivedCommand,
    timeline: &mut Timeline,
    updates: &mut Option<ClipUpdateMap>,
) -> EsotereelResult<()> {
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

            let new_clip = timeline
                .layers
                .new_clip(*position, *duration, clip_data, translates);

            clip_add(timeline, &key, new_clip, updates);
        }
    }
    Ok(())
}
