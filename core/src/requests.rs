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

                state.project = Some(Arc::new(RwLock::new(new_project)));

                let cmd = Response::ProjectMeta { timelines };
                state.network.send(client_id, &cmd);
            }
        }
        ArchivedRequest::ProjectAll => {
            let project_arc = {
                let state = state.lock().expect("mutex poisoned");

                state.project.as_ref().map(Arc::clone)
            };

            let project_arc = project_arc.ok_or_else(|| EsotereelError::ProjectNotFound)?;

            let project_guard = project_arc.write().unwrap();

            let timelines = project_guard.timelines_meta();

            {
                let state = state.lock().expect("mutex poisoned");

                // クライアントのビューを初期化：すべてのタイムラインを見ているとみなす
                for timeline in &timelines {
                    state
                        .network
                        .update_client_view(client_id, timeline.id, i64::MIN..i64::MAX);
                }

                let cmd = Response::ProjectMeta { timelines };

                state.network.send(client_id, &cmd);
            }
        }
        ArchivedRequest::Command {
            command,
            timeline_id,
        } => {
            let project_arc = {
                let project_guard = state.lock().expect("mutex poisoned");

                project_guard.project.as_ref().map(Arc::clone)
            };

            let project_arc = project_arc.ok_or_else(|| EsotereelError::ProjectNotFound)?;

            let mut project_guard = project_arc.write().unwrap();

            let command: CommandRequest = command.deserialize(&mut rkyv::Infallible).unwrap();

            info!("request: {:?}", command);

            let history = command_to_history(&mut project_guard, *timeline_id, command)?;
            handle_command_action(&mut project_guard, *timeline_id, &history)?;

            info!("history: {:?}", history);

            drop(project_guard);

            let state_guard = state.lock().expect("mutex poisoned");
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
                .path_to_stream
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

            let state = state.lock().expect("mutex poisoned");

            let mut streamer = state
                .streams
                .get_mut(resource_id)
                .ok_or_else(|| EsotereelError::StreamNotFound(*resource_id))?;

            let generation = streamer.next_generation();

            let to_send = streamer.fetch_stream_data_batch(*resource_id, ranges, generation)?;
            for res in to_send {
                state.network.send(client_id, &res);
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
            let state = state.lock().expect("mutex poisoned");

            let range: Range<i64> = range.deserialize(&mut rkyv::Infallible).unwrap();

            // クライアントの表示範囲をサーバーに記憶させる
            state
                .network
                .update_client_view(client_id, *timeline_key, range.clone());

            // 範囲内のクリップ送信
            let project_arc = state.project.as_ref();
            let project_arc = project_arc.ok_or_else(|| EsotereelError::ProjectNotFound)?;

            let project_arc = Arc::clone(project_arc);

            let project_guard = project_arc.write().unwrap();

            let timeline = project_guard
                .timeline(*timeline_key)
                .ok_or(EsotereelError::TimelineNotFound(*timeline_key))?;

            let clips = timeline
                .query_range(range)
                .into_iter()
                .map(|(layer, clip)| (layer.id, clip.clone()))
                .collect();

            state.network.send(
                client_id,
                &Response::UpdateClip {
                    timeline_id: *timeline_key,
                    clips,
                },
            );
        }
        ArchivedRequest::DebugFetchProjectStruct => {
            let state = state.lock().expect("mutex poisoned");
            let project_arc = state.project.as_ref();
            let Some(project_arc) = project_arc else {
                state
                    .network
                    .send(client_id, &&Response::DebugProjectStruct(None));
                anyhow::bail!(EsotereelError::ProjectNotFound)
            };

            let project_arc = Arc::clone(&project_arc);
            let project_guard = project_arc.write().unwrap();

            let str = format!("{:#?}", project_guard);

            state
                .network
                .send(client_id, &&Response::DebugProjectStruct(Some(str)));
        }
    }
    Ok(())
}
