use std::{
    ops::Range,
    sync::{Arc, Mutex, RwLock},
};

use esotereel_lib::{
    StreamState,
    decode::videostreamer::VideoStreamer,
    project::{Project, command::CommandRequest},
    requests::ArchivedRequest,
    responces::Response,
    util::result::{EsotereelError, EsotereelResult},
};
use log::info;
use rkyv::Deserialize;

use crate::{
    project::commands::{command_to_history, handle_command_action},
    state::ServerState,
};

pub fn on_request_receive(
    request: &ArchivedRequest,
    client_id: u32,
    state: &Mutex<ServerState>,
) -> EsotereelResult<()> {
    match request {
        ArchivedRequest::Test => {}
        ArchivedRequest::NewProject => {
            log::info!(
                "Server: Handling NewProject request from client {}",
                client_id
            );

            let mut new_project = Project::new();
            let placeholder_fps = 60.0;
            let timeline_key = new_project.insert_timeline(placeholder_fps);

            assert!(timeline_key == 0, "main timeline should be first");

            // Timeline::new()ですでに4つのデフォルトレイヤーを作成しているので、
            // ここで重複して挿入する必要はない

            let timelines = new_project.timelines_meta();

            {
                let mut state = state.lock().expect("mutex poisoned");
                for timeline in &timelines {
                    state
                        .network
                        .update_client_view(client_id, timeline.id, i64::MIN..i64::MAX);
                }

                // クライアントのビューを初期化：すべてのタイムラインを見ているとみなす

                state.project = Arc::new(RwLock::new(Some(new_project)));

                let cmd = Response::ProjectMeta { timelines };
                state.network.send(client_id, &cmd);
            }
        }
        ArchivedRequest::ProjectAll => {
            let state_guard = state.lock().expect("mutex poisoned");
            let mut project_guard = state_guard.project.write().expect("mutex poisoned");
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let timelines = project.timelines_meta();

            {
                // クライアントのビューを初期化：すべてのタイムラインを見ているとみなす
                for timeline in &timelines {
                    state_guard.network.update_client_view(
                        client_id,
                        timeline.id,
                        i64::MIN..i64::MAX,
                    );
                }

                let cmd = Response::ProjectMeta { timelines };

                state_guard.network.send(client_id, &cmd);
            }
        }
        ArchivedRequest::Command {
            command,
            timeline_id,
        } => {
            let state_guard = state.lock().expect("mutex poisoned");
            let mut project_guard = state_guard.project.write().expect("mutex poisoned");
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let command: CommandRequest = command.deserialize(&mut rkyv::Infallible).unwrap();

            info!("request: {:?}", command);

            let history = command_to_history(project, *timeline_id, command)?;
            handle_command_action(project, *timeline_id, &history)?;

            info!("history: {:?}", history);

            drop(project);

            state_guard.network.notify_dirty();
        }
        ArchivedRequest::InitStream { path } => {
            let path = path.as_ref();

            let streamer = VideoStreamer::new(path).map_err(|e| {
                EsotereelError::IoError(format!("Failed to open video stream: {:?}", e))
            })?;

            let mut state = state.lock().expect("mutex poisoned");

            let resource_id = state.get_or_create_resource_id(path);

            let res = streamer.get_init_packet(path, resource_id);

            state.streams.insert(resource_id, streamer);
            state
                .stream_state_map
                .insert(path.to_owned(), StreamState::Loaded(resource_id));

            state.network.send(client_id, &res);

            log::info!(
                "Sent StreamMetadata for resource_id: {} ({})",
                resource_id,
                path
            );
        }
        ArchivedRequest::FetchStreamData {
            resource_id,
            ranges,
        } => {
            log::info!(
                "Received FetchStreamData request for resource_id: {} ranges: {:?}",
                resource_id,
                ranges
            );

            let ranges: Vec<Range<f64>> = ranges.deserialize(&mut rkyv::Infallible).unwrap();

            let state_guard = state.lock().expect("mutex poisoned");

            let mut streamer = state_guard
                .streams
                .get_mut(resource_id)
                .ok_or_else(|| EsotereelError::StreamNotFound(*resource_id))?;

            let generation = streamer.next_generation();

            let to_send = streamer.fetch_stream_data_batch(*resource_id, ranges, generation)?;
            for res in to_send {
                state_guard.network.send(client_id, &res);
            }
        }
        ArchivedRequest::FetchClipsInRange {
            timeline_id: timeline_key,
            range,
        } => {
            log::info!(
                "Received FetchClipsInRange request for timeline_key: {} in:{:?}",
                timeline_key,
                range
            );
            let state_guard = state.lock().expect("mutex poisoned");

            let range: Range<i64> = range.deserialize(&mut rkyv::Infallible).unwrap();

            // クライアントの表示範囲をサーバーに記憶させる
            state_guard
                .network
                .update_client_view(client_id, *timeline_key, range.clone());

            // 範囲内のクリップ送信
            let mut project_guard = state_guard.project.write().expect("mutex poisoned");
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let timeline = project
                .timeline_ref(*timeline_key)
                .ok_or(EsotereelError::TimelineNotFound(*timeline_key))?;

            let clips = timeline
                .query_range(range)
                .into_iter()
                .map(|(layer, clip)| (layer.id, clip.clone()))
                .collect();

            state_guard.network.send(
                client_id,
                &Response::UpdateClip {
                    timeline_id: *timeline_key,
                    clips,
                },
            );
        }
        ArchivedRequest::DebugFetchProjectStruct => {
            let state_guard = state.lock().expect("mutex poisoned");
            let mut project_guard = state_guard.project.write().expect("mutex poisoned");
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            state_guard
                .network
                .send(client_id, &Response::DebugProjectStruct(None));

            let str = format!("{:#?}", project);

            state_guard
                .network
                .send(client_id, &&Response::DebugProjectStruct(Some(str)));
        }
    }
    Ok(())
}
