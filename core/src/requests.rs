use std::{
    ops::Range,
    sync::{Arc, Mutex, RwLock},
};

use esotereel_lib::{
    StreamState,
    decode::videostreamer::VideoStreamer,
    plugin::{NamespacedID, script::api::PluginActionContext},
    project::{Project, command::CommandRequest, ids::TimelineId},
    requests::{ArchivedRequest, Request},
    responces::Response,
    util::result::{EsotereelError, EsotereelResult, LockExt},
};
use rkyv::Deserialize;

use crate::{
    project::{
        commands::command_to_history,
        history::{HistoryStack, execute_command_with_history, redo_command, undo_command},
    },
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
                let mut state = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
                for timeline in &timelines {
                    state
                        .network
                        .update_client_view(client_id, timeline.id, i64::MIN..i64::MAX);
                }

                // クライアントのビューを初期化：すべてのタイムラインを見ているとみなす

                state.project = Arc::new(RwLock::new(Some(new_project)));
                state
                    .histories
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?
                    .clear();

                let cmd = Response::ProjectMeta { timelines };
                state.network.send(client_id, &cmd);
            }
        }
        ArchivedRequest::ProjectAll => {
            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            let mut project_guard = state_guard.project.write_or_err()?;
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
        ArchivedRequest::Command { commands } => {
            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            let project_arc = &state_guard.project;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            for (timeline_id, command) in commands.iter() {
                let command: CommandRequest = command.deserialize(&mut rkyv::Infallible).unwrap();

                log::debug!(target:"core_packet_request","request: {:?}", command);

                match command_to_history(project, *timeline_id, command.clone()) {
                    Ok(command_history) => {
                        let mut histories = state_guard.histories.lock().map_err(|e| {
                            anyhow::Error::from(EsotereelError::LockError(e.to_string()))
                        })?;
                        let history = histories
                            .entry(*timeline_id)
                            .or_insert_with(|| HistoryStack::default());
                        if let Err(e) = execute_command_with_history(
                            project,
                            *timeline_id,
                            command_history.clone(),
                            history,
                        ) {
                            log::error!("Command execution error: {:?}", e);
                        } else {
                            log::debug!(target:"core_packet_history","history: {:?}", command_history);
                        }
                    }
                    Err(e) => {
                        log::error!("Command rejected: {:?}", e);
                        // クライアントに拒否を通知し、サーバー側の正しい状態で同期し直すよう促す
                        let rejections = build_command_rejected_responses(
                            project,
                            *timeline_id,
                            client_id,
                            &command,
                        );
                        for resp in rejections {
                            state_guard.network.send(client_id, &resp);
                        }
                    }
                }
            }

            drop(project_guard);
            state_guard.network.notify_dirty();
        }
        ArchivedRequest::InitStream { path } => {
            let path = path.as_ref();

            let streamer = VideoStreamer::new(path).map_err(|e| {
                EsotereelError::IoError(format!("Failed to open video stream: {:?}", e))
            })?;

            let mut state = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

            let resource_id = state.get_or_create_resource_id(path);

            let res = streamer.get_init_packet(path, resource_id);

            state.streams.insert(resource_id, streamer);
            state
                .stream_state_map
                .insert(path.to_owned(), StreamState::Loaded(resource_id));

            state.network.send(client_id, &res);

            log::debug!(target: "core_stream_packet_handler",
                "Sent StreamMetadata for resource_id: {} ({})",
                resource_id,
                path
            );
        }
        ArchivedRequest::FetchStreamData {
            resource_id,
            ranges,
        } => {
            log::debug!(target: "core_stream_packet_handler",
                "Received FetchStreamData request for resource_id: {} ranges: {:?}",
                resource_id,
                ranges
            );

            let ranges: Vec<Range<f64>> = ranges.deserialize(&mut rkyv::Infallible).unwrap();

            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

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
            log::debug!(target: "core_stream_packet_handler",
                "Received FetchClipsInRange request for timeline_key: {} in:{:?}",
                timeline_key,
                range
            );
            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

            let range: Range<i64> = range.deserialize(&mut rkyv::Infallible).unwrap();

            // クライアントの表示範囲をサーバーに記憶させる
            state_guard
                .network
                .update_client_view(client_id, *timeline_key, range.clone());

            // 範囲内のクリップ送信
            let mut project_guard = state_guard.project.write_or_err()?;
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
            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            let mut project_guard = state_guard.project.write_or_err()?;
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
        ArchivedRequest::ToolbarAction {
            button_id,
            func_name,
            run_on: _,
            timeline_id,
        } => {
            let button_id = NamespacedID::parse(button_id.as_ref())?;
            let func_name = func_name.as_ref();

            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

            let plugin_id = button_id.plugin_id();

            // PluginActionContextを作成
            let project = Arc::clone(&state_guard.project);

            let context =
                PluginActionContext::new(Arc::clone(&state_guard.network), project, *timeline_id);

            // スクリプト実行
            state_guard
                .common
                .plugin_loader
                .read_or_err()?
                .call_script::<()>(
                    &state_guard.common.script_engine,
                    &plugin_id,
                    func_name,
                    (context.clone(),),
                )
                .map_err(|e| anyhow::anyhow!("Failed to execute toolbar script: {}", e))?;

            // スクリプト内で積まれた Request を回収して処理
            let packets = context.take_packets();
            for packet in packets {
                match packet {
                    Request::Undo { timeline_id } => {
                        let project_arc = &state_guard.project;
                        let mut project_guard = project_arc.write_or_err()?;
                        let project = project_guard
                            .as_mut()
                            .ok_or(EsotereelError::ProjectNotFound)?;

                        let mut histories = state_guard.histories.lock().map_err(|e| {
                            anyhow::Error::from(EsotereelError::LockError(e.to_string()))
                        })?;
                        let history = histories
                            .entry(timeline_id)
                            .or_insert_with(|| HistoryStack::default());

                        undo_command(project, timeline_id, history)?;
                    }
                    Request::Redo { timeline_id } => {
                        let project_arc = &state_guard.project;
                        let mut project_guard = project_arc.write_or_err()?;
                        let project = project_guard
                            .as_mut()
                            .ok_or(EsotereelError::ProjectNotFound)?;

                        let mut histories = state_guard.histories.lock().map_err(|e| {
                            anyhow::Error::from(EsotereelError::LockError(e.to_string()))
                        })?;
                        let history = histories
                            .entry(timeline_id)
                            .or_insert_with(|| HistoryStack::default());

                        redo_command(project, timeline_id, history)?;
                    }
                    _ => {
                        log::warn!("Unhandled packet type in toolbar action: {:?}", packet);
                    }
                }
            }

            log::debug!(target: "core_toolbar_action",
                "Core: Executed toolbar action {} from plugin {}",
                func_name,
                plugin_id
            );
        }
        ArchivedRequest::Undo { timeline_id } => {
            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            let project_arc = &state_guard.project;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let mut histories = state_guard
                .histories
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            let history = histories
                .entry(*timeline_id)
                .or_insert_with(|| HistoryStack::default());

            undo_command(project, *timeline_id, history)?;
            // must send to client
            state_guard.network.notify_dirty();
        }
        ArchivedRequest::Redo { timeline_id } => {
            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            let project_arc = &state_guard.project;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let mut histories = state_guard
                .histories
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            let history = histories
                .entry(*timeline_id)
                .or_insert_with(|| HistoryStack::default());

            redo_command(project, *timeline_id, history)?;
            // must send to client
            state_guard.network.notify_dirty();
        }
    }
    Ok(())
}

/// コマンドが拒否された場合にクライアントへ送信する `CommandRejected` レスポンスを構築する。
/// クライアントはこれを受け取り、サーバー側の正しいクリップ状態に同期し直す。
fn build_command_rejected_responses(
    project: &Project,
    timeline_id: TimelineId,
    client_id: u32,
    command: &CommandRequest,
) -> Vec<Response> {
    let Some(timeline) = project.timeline_ref(timeline_id) else {
        return vec![];
    };

    match command {
        CommandRequest::ClipsMove { clips } => clips
            .iter()
            .filter_map(|ctx| {
                let (clip, layer) = timeline.get_clip_and_layer(ctx.clip_id)?;
                if clip.position != ctx.base_position || clip.duration != ctx.base_duration {
log::warn!(
                            "stale move request: client ({}) base ({}, {}) != server current ({}, {})",
                            client_id,
                            ctx.base_position, ctx.base_duration, clip.position, clip.duration
                        );

                    Some(Response::CommandRejected {
                        timeline_id,
                        clip_id: ctx.clip_id,
                        current_position: clip.position,
                        current_duration: clip.duration,
                        current_layer_id: layer,
                    })
                } else {
                    None
                }
            })
            .collect(),
        CommandRequest::ClipsResize { clips } => clips
            .iter()
            .filter_map(|ctx| {
                let clip = timeline.get_clip(ctx.clip_id)?;
                if clip.position != ctx.base_position || clip.duration != ctx.base_duration {
                    log::warn!(
                        "stale resize request: client ({}) base ({}, {}) != server current ({}, {})",
                        client_id,
                        ctx.base_position,
                        ctx.base_duration,
                        clip.position,
                        clip.duration
                    );

                    Some(Response::CommandRejected {
                        timeline_id,
                        clip_id: ctx.clip_id,
                        current_position: clip.position,
                        current_duration: clip.duration,
                        current_layer_id: 0,
                    })
                } else {
                    None
                }
            })
            .collect(),
        _ => vec![],
    }
}
