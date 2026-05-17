use std::{collections::HashMap, ops::Range, sync::Arc};

use esotereel_lib::{
    StreamState,
    decode::videostreamer::VideoStreamer,
    project::{ClipUpdateMap, Project, util::ProjectMutExt},
    requests::ArchivedRequest,
    responces::Response,
    util::result::{EsotereelError, EsotereelResult, LockExt},
};
use rkyv::Deserialize;

use crate::{network::ServerNetworkHandler, project::commands::handle_command_action};

pub fn on_request_receive(
    request: &ArchivedRequest,
    client_id: u32,
    network: &Arc<ServerNetworkHandler>,
) -> EsotereelResult<()> {
    let mut app_state = network.app_state.lock().expect("mutex poisoned");

    match request {
        ArchivedRequest::Test => {}
        ArchivedRequest::NewProject => {
            log::info!(
                "Server: Handling NewProject request from client {}",
                client_id
            );
            let mut lock = app_state.project.write_or_err()?;
            let new_project = Project::new();

            // new_project.debug_add_clips(0, 0);

            let cmd = Response::ProjectAll {
                project: new_project.clone(),
            };

            *lock = Some(new_project);
            network.send(client_id, &cmd);
        }
        ArchivedRequest::ProjectAll => {
            let mut lock = app_state.project.write_or_err()?;
            let project = lock.project_mut_err()?;

            let cmd = Response::ProjectAll {
                project: project.clone(),
            };

            network.send(client_id, &cmd);
        }
        ArchivedRequest::Command {
            command,
            timeline_idx,
        } => {
            let timeline_idx = *timeline_idx as usize;

            let mut lock = app_state.project.write_or_err()?;
            let project = lock.project_mut_err()?;

            let mut updates: Option<ClipUpdateMap> = Some(HashMap::new());
            let timeline = project.get_timeline_mut(timeline_idx)?;

            handle_command_action(command, timeline, &mut updates)?;

            // send updates
            let updates = updates.unwrap_or_default();
            let cmd = Response::ClipUpdates {
                timeline_type: timeline_idx,
                updates,
            };
            network.send_all(&cmd);
        }
        ArchivedRequest::InitStream { path } => {
            let path = path.as_ref();

            let streamer = VideoStreamer::new(path).map_err(|e| {
                EsotereelError::IoError(format!("Failed to open video stream: {:?}", e))
            })?;

            let resource_id = app_state.get_or_create_resource_id(path);

            let res = streamer.get_init_packet(path, resource_id);

            app_state.streams.insert(resource_id, streamer);
            app_state
                .path_to_stream
                .insert(path.to_owned(), StreamState::Loaded(resource_id));

            network.send(client_id, &res);

            log::info!(
                "Sent StreamMetadata for resource_id: {} ({})",
                resource_id,
                path
            );
        }
        ArchivedRequest::FetchStreamData {
            resource_id,
            seek_range_sec,
        } => {
            log::info!(
                "Received FetchStreamData request for resource_id: {} at:{:?}",
                resource_id,
                seek_range_sec
            );

            let seek_range_sec: Range<f64> =
                seek_range_sec.deserialize(&mut rkyv::Infallible).unwrap();

            let mut streamer = app_state
                .streams
                .get_mut(resource_id)
                .ok_or_else(|| EsotereelError::StreamNotFound(*resource_id))?;

            let to_send = streamer.fetch_stream_data(*resource_id, seek_range_sec)?;

            for res in to_send {
                network.send(client_id, &res);
            }
        }
    }
    Ok(())
}
