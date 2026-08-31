use std::{
    ops::Range,
    sync::{Arc, RwLock},
};

use esotereel_lib::{
    StreamState,
    decode::videostreamer::VideoStreamer,
    project::Project,
    requests::ArchivedRequest,
    responces::Response,
    util::result::{EsotereelError, EsotereelResult},
};
use rkyv::Deserialize;

use crate::{network::ServerNetworkHandler, project::commands::handle_command_action};

pub fn on_request_receive(
    request: &ArchivedRequest,
    client_id: u32,
    network: &Arc<ServerNetworkHandler>,
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

            // クライアントのビューを初期化：すべてのタイムラインを見ているとみなす
            for timeline in &timelines {
                network.update_client_view(client_id, timeline.id, i64::MIN..i64::MAX);
            }

            let cmd = Response::ProjectMeta { timelines };

            {
                let mut app_state = network.app_state.lock().expect("mutex poisoned");
                app_state.project = Some(Arc::new(RwLock::new(new_project)));
            }

            network.send(client_id, &cmd);
        }
        ArchivedRequest::ProjectAll => {
            let app_state = network.app_state.lock().expect("mutex poisoned");

            let project_arc = app_state.project.as_ref().cloned();
            let Some(project_arc) = project_arc else {
                anyhow::bail!(EsotereelError::ProjectNotFound)
            };

            let project = project_arc.write().unwrap();

            let timelines = project.timelines_meta();

            // クライアントのビューを初期化：すべてのタイムラインを見ているとみなす
            for timeline in &timelines {
                network.update_client_view(client_id, timeline.id, i64::MIN..i64::MAX);
            }

            let cmd = Response::ProjectMeta { timelines };

            network.send(client_id, &cmd);
        }
        ArchivedRequest::Command {
            command,
            timeline_id,
        } => {
            let app_state = network.app_state.lock().expect("mutex poisoned");

            let project_arc = app_state.project.as_ref().cloned();
            let Some(project_arc) = project_arc else {
                anyhow::bail!(EsotereelError::ProjectNotFound)
            };

            let mut project = project_arc.write().unwrap();

            handle_command_action(command, &mut project, *timeline_id)?;

            drop(app_state);

            network.notify_dirty();
        }
        ArchivedRequest::InitStream { path } => {
            let path = path.as_ref();

            let streamer = VideoStreamer::new(path).map_err(|e| {
                EsotereelError::IoError(format!("Failed to open video stream: {:?}", e))
            })?;

            let mut app_state = network.app_state.lock().expect("mutex poisoned");

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
            ranges,
        } => {
            log::info!(
                "Received FetchStreamData request for resource_id: {} ranges: {:?}",
                resource_id,
                ranges
            );

            let ranges: Vec<Range<f64>> = ranges.deserialize(&mut rkyv::Infallible).unwrap();

            let app_state = network.app_state.lock().expect("mutex poisoned");

            let mut streamer = app_state
                .streams
                .get_mut(resource_id)
                .ok_or_else(|| EsotereelError::StreamNotFound(*resource_id))?;

            let generation = streamer.next_generation();

            let to_send = streamer.fetch_stream_data_batch(*resource_id, ranges, generation)?;
            for res in to_send {
                network.send(client_id, &res);
            }
        }
        ArchivedRequest::FetchClipsInRange {
            timeline_key,
            range,
        } => {
            log::info!(
                "Received FetchClipsInRange request for timeline_key: {} in:{:?}",
                timeline_key,
                range
            );
            let app_state = network.app_state.lock().expect("mutex poisoned");

            let range: Range<i64> = range.deserialize(&mut rkyv::Infallible).unwrap();

            // クライアントの表示範囲をサーバーに記憶させる
            network.update_client_view(client_id, *timeline_key, range.clone());

            // 範囲内のクリップ送信
            let project_arc = app_state.project.as_ref().cloned();
            let Some(project_arc) = project_arc else {
                anyhow::bail!(EsotereelError::ProjectNotFound)
            };

            let project = project_arc.write().unwrap();

            let timeline = project
                .timeline(*timeline_key)
                .ok_or(EsotereelError::InvalidTimeline)?;

            let clips = timeline
                .query_range(range)
                .into_iter()
                .map(|(layer, clip)| (layer.id, clip.clone()))
                .collect();

            network.send(
                client_id,
                &Response::UpdateClip {
                    timeline_id: *timeline_key,
                    clips,
                },
            );
        }
        ArchivedRequest::DebugFetchProjectStruct => {
            let app_state = network.app_state.lock().expect("mutex poisoned");
            let project_arc = app_state.project.as_ref().cloned();
            let Some(project_arc) = project_arc else {
                network.send(client_id, &&Response::DebugProjectStruct(None));
                anyhow::bail!(EsotereelError::ProjectNotFound)
            };

            let project = project_arc.write().unwrap();

            let str = format!("{:#?}", project);

            network.send(client_id, &&Response::DebugProjectStruct(Some(str)));
        }
    }
    Ok(())
}
