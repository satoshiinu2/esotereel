use std::collections::HashMap;

use nomyoedit_lib::{
    command::ArchivedCommand,
    project::{Project, clip::Clip},
    responce::{Response, send_response},
    types::ClipMoveCtx,
};
use rkyv::Deserialize as _;

use crate::{PROJECT, project::clip_move_mul_core};

pub fn on_command_recveve(command: &ArchivedCommand) -> Result<(), String> {
    match command {
        ArchivedCommand::Test => {}
        ArchivedCommand::NewProject => {
            let mut lock = PROJECT.write().unwrap();
            let new_project = Project::new();

            let cmd = Response::ProjectAll {
                project: new_project.clone(),
            };

            *lock = Some(new_project);
            send_response(cmd);
        }
        ArchivedCommand::ClipsMove {
            timeline_idx: timeline_type,
            clips,
        } => {
            let timeline_type = *timeline_type as usize;

            let mut lock = PROJECT.write().unwrap();
            let project = lock
                .as_mut()
                .ok_or(nomyoedit_lib::ERROR_NO_PROJECT_LOADED)?;
            let moved_clips: Vec<ClipMoveCtx> = clips.deserialize(&mut rkyv::Infallible).unwrap();
            let mut updates: HashMap<usize, Vec<Clip>> = HashMap::new();
            let timeline = project.get_timeline_mut(timeline_type)?;

            clip_move_mul_core(timeline, moved_clips, &mut updates);

            // send updates
            let cmd = Response::ClipUpdates {
                timeline_type,
                updates,
            };
            send_response(cmd);
        }
    }
    Ok(())
}
