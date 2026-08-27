use std::{ops::Range, sync::Arc};

use esotereel_lib::{
    StreamState,
    decode::videostreamer::VideoStreamer,
    project::Project,
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

            *lock = Some(new_project);
            network.send(client_id, &cmd);
        }
        ArchivedRequest::ProjectAll => {
            let mut lock = app_state.project.write_or_err()?;
            let project = lock.as_mut().unwrap();

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
            let timeline_id = *timeline_id;

            {
                let mut lock = app_state.project.write_or_err()?;
                let mut project = lock.as_mut().unwrap();

                handle_command_action(command, &mut project, timeline_id)?;
            }
            drop(app_state);

            network.notify_dirty();
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
            ranges,
        } => {
            log::info!(
                "Received FetchStreamData request for resource_id: {} ranges: {:?}",
                resource_id,
                ranges
            );

            let ranges: Vec<Range<f64>> = ranges.deserialize(&mut rkyv::Infallible).unwrap();

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

            let range: Range<i64> = range.deserialize(&mut rkyv::Infallible).unwrap();

            // クライアントの表示範囲をサーバーに記憶させる
            network.update_client_view(client_id, *timeline_key, range.clone());

            let project_arc = app_state.project.clone();
            drop(app_state);

            // 範囲内のクリップ送信（読み取りのみなので read ロック）
            let lock = project_arc.read_or_err()?;
            let project = lock.as_ref().ok_or(EsotereelError::InvalidTimeline)?;
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
            let mut lock = app_state.project.write_or_err()?;
            let project = lock.as_mut().unwrap();

            let str = format!("{:#?}", project);

            network.send(client_id, &&Response::DebugProjectStruct(str));
        }
    }
    Ok(())
}
