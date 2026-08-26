use esotereel_lib::project::{
    Project, clip::ClipData, commands::ArchivedCommand, ids::TimelineId, transform::ClipTranslates,
};

use rkyv::Deserialize as _;

use crate::project::{clip_add_core, clip_move_mul_core};

pub fn handle_command_action(
    request: &ArchivedCommand,
    project: &mut Project,
    timeline_id: TimelineId,
) -> anyhow::Result<()> {
    match request {
        ArchivedCommand::ClipsMove { clips } => {
            clip_move_mul_core(project, timeline_id, clips.as_slice())?
        }
        ArchivedCommand::AddClip {
            layer_id,
            position,
            duration,
            clip_data,
            translates,
        } => {
            let clip_data: ClipData = clip_data.deserialize(&mut rkyv::Infallible).unwrap();
            let translates: ClipTranslates = translates.deserialize(&mut rkyv::Infallible).unwrap();

            clip_add_core(
                project,
                timeline_id,
                *layer_id,
                *position,
                *duration,
                clip_data,
                translates,
            )?
        }
        ArchivedCommand::AddLayer {
            parent_layer_id,
            insert_index,
            name,
            is_folder,
        } => {
            let parent_layer_id = parent_layer_id.as_ref().copied();
            let insert_index = insert_index.as_ref().map(|x| *x as usize);

            project.insert_layer_in_timeline(
                timeline_id,
                parent_layer_id,
                insert_index,
                name.to_string(),
                *is_folder,
            )?;
        }
    };
    Ok(())
}
