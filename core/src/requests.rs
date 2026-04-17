use std::collections::HashMap;

use esotereel_lib::{
    project::{Project, util::ProjectExt},
    requests::ArchivedRequest,
    responces::{Response, send_response},
    util::error::{EsotereelResult, LockExt},
};

use crate::{
    PROJECT,
    project::{ClipUpdateMap, commands::handle_command_action},
};

pub fn on_request_recveve(request: &ArchivedRequest) -> EsotereelResult<()> {
    match request {
        ArchivedRequest::Test => {}
        ArchivedRequest::NewProject => {
            let mut lock = PROJECT.write_err()?;
            let new_project = Project::new();

            let cmd = Response::ProjectAll {
                project: new_project.clone(),
            };

            *lock = Some(new_project);
            send_response(cmd);
        }
        ArchivedRequest::Command {
            command,
            timeline_idx,
        } => {
            let timeline_idx = *timeline_idx as usize;

            let mut lock = PROJECT.write_err()?;
            let project = lock.project_err()?;

            let mut updates: ClipUpdateMap = Some(HashMap::new());
            let timeline = project.get_timeline_mut(timeline_idx)?;

            handle_command_action(command, timeline, &mut updates)?;

            // send updates
            let updates = updates.unwrap_or_default();
            let cmd = Response::ClipUpdates {
                timeline_type: timeline_idx,
                updates,
            };
            send_response(cmd);
        }
    }
    Ok(())
}
