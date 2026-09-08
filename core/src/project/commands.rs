use esotereel_lib::{
    project::{
        Project,
        command::{ClipMoveHistoryCtx, CommandHistory, CommandRequest},
        ids::TimelineId,
    },
    util::result::EsotereelError,
};

use crate::project::{clip_add_core, clip_move_mul_core};

pub fn command_to_history(
    project: &mut Project,
    timeline_id: TimelineId,
    request: CommandRequest,
) -> anyhow::Result<CommandHistory> {
    let history = match request {
        CommandRequest::ClipsMove { clips } => {
            let timeline = project
                .timeline_mut(timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

            let entries: anyhow::Result<Vec<_>> = clips
                .into_iter()
                .map(|ctx| -> anyhow::Result<ClipMoveHistoryCtx> {
                    let (clip, layer) = timeline
                        .get_clip_and_layer(ctx.clip_id)
                        .ok_or(EsotereelError::ClipNotFound(ctx.clip_id))?;

                    Ok(ClipMoveHistoryCtx {
                        clip_id: ctx.clip_id,
                        old_position: clip.position,
                        old_duration: clip.duration,
                        old_layer_id: layer,
                        new_position: ctx.new_position,
                        new_duration: ctx.new_duration,
                        new_layer_id: ctx.new_layer_id,
                    })
                })
                .collect();
            CommandHistory::ClipsMove { clips: entries? }
        }
        CommandRequest::AddClip {
            layer_id,
            position,
            duration,
            clip_data,
            translates,
        } => CommandHistory::AddClip {
            layer_id,
            position,
            duration,
            clip_data,
            translates,
        },
        CommandRequest::AddLayer {
            parent_folder_id: parent_layer_id,
            insert_index,
            name,
        } => CommandHistory::AddLayer {
            parent_layer_id,
            insert_index,
            name,
        },
        CommandRequest::AddFolder {
            parent_folder_id: parent_layer_id,
            insert_index,
            name,
        } => CommandHistory::AddFolder {
            parent_layer_id,
            insert_index,
            name,
        },
    };

    Ok(history)
}

pub fn handle_command_action(
    project: &mut Project,
    timeline_id: TimelineId,
    command: &CommandHistory,
) -> anyhow::Result<()> {
    match command {
        CommandHistory::ClipsMove { clips } => {
            clip_move_mul_core(project, timeline_id, clips.as_slice())?
        }
        CommandHistory::AddClip {
            layer_id,
            position,
            duration,
            clip_data,
            translates,
        } => clip_add_core(
            project,
            timeline_id,
            *layer_id,
            *position,
            *duration,
            clip_data.clone(),
            translates.clone(),
        )?,
        CommandHistory::AddLayer {
            parent_layer_id,
            insert_index,
            name,
        } => {
            let parent_layer_id = parent_layer_id.as_ref().copied();
            let insert_index = insert_index.as_ref().map(|x| *x as usize);

            project.insert_layer_in_timeline(
                timeline_id,
                parent_layer_id,
                insert_index,
                name.to_string(),
            )?;
        }
        CommandHistory::AddFolder {
            parent_layer_id,
            insert_index,
            name,
        } => {
            let parent_layer_id = parent_layer_id.as_ref().copied();
            let insert_index = insert_index.as_ref().map(|x| *x as usize);

            project.insert_folder_in_timeline(
                timeline_id,
                parent_layer_id,
                insert_index,
                name.to_string(),
            )?;
        }
    };
    Ok(())
}
