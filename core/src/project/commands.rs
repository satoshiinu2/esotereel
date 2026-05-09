use esotereel_lib::{
    project::{clipdata::ClipData, commands::ArchivedCommand, timeline::Timeline},
    util::result::EsotereelResult,
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
            layer_idx,
            position,
            duration,
            clip_data,
        } => {
            let clip_data: ClipData = clip_data.deserialize(&mut rkyv::Infallible).unwrap();

            let new_clip = timeline.new_clip(*position, *duration, clip_data);

            clip_add(timeline, *layer_idx, new_clip, updates);
        }
    }
    Ok(())
}
