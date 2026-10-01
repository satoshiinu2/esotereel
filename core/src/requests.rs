use std::{
    ops::Range,
    sync::{Arc, Mutex, RwLock},
};

use esotereel_lib::{
    StreamState,
    decode::videostreamer::VideoStreamer,
    plugin::script::api::PluginActionContext,
    project::{Project, command::CommandRequest},
    requests::ArchivedRequest,
    responces::Response,
    util::result::{EsotereelError, EsotereelResult},
};
use log::info;
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
                let mut state = state.lock().expect("mutex poisoned");
                for timeline in &timelines {
                    state
                        .network
                        .update_client_view(client_id, timeline.id, i64::MIN..i64::MAX);
                }

                // クライアントのビューを初期化：すべてのタイムラインを見ているとみなす

                state.project = Arc::new(RwLock::new(Some(new_project)));
                state.histories.clear();

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
        ArchivedRequest::Command { commands } => {
            let mut state_guard = state.lock().expect("mutex poisoned");
            let project_arc = state_guard.project.clone();
            let mut project_guard = project_arc.write().expect("mutex poisoned");
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            for (timeline_id, command) in commands.iter() {
                let command: CommandRequest = command.deserialize(&mut rkyv::Infallible).unwrap();

                info!("request: {:?}", command);

                let command_history = command_to_history(project, *timeline_id, command)?;
                let history = state_guard
                    .histories
                    .entry(*timeline_id)
                    .or_insert_with(|| HistoryStack::new(100));
                execute_command_with_history(
                    project,
                    *timeline_id,
                    command_history.clone(),
                    history,
                )?;

                info!("history: {:?}", command_history);
            }

            drop(project_guard);
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
        ArchivedRequest::ToolbarAction {
            button_id,
            func_name,
            run_on: _,
            timeline_id,
        } => {
            let button_id = button_id.as_ref();
            let func_name = func_name.as_ref();

            let mut state_guard = state.lock().expect("mutex poisoned");

            // 標準のundo/redoはRhaiスクリプト経由ではなく、サーバーのコマンド履歴を直接操作する。
            // スクリプト関数はActionContext引数を受け取らないため、従来の呼び出しではarityエラーになっていた。
            if func_name == "toolbar_undo" || func_name == "toolbar_redo" {
                let project_arc = state_guard.project.clone();
                let mut project_guard = project_arc.write().expect("mutex poisoned");
                let project = project_guard
                    .as_mut()
                    .ok_or(EsotereelError::ProjectNotFound)?;
                let history = state_guard
                    .histories
                    .entry(*timeline_id)
                    .or_insert_with(|| HistoryStack::new(100));

                if func_name == "toolbar_undo" {
                    undo_command(project, *timeline_id, history)?;
                } else {
                    redo_command(project, *timeline_id, history)?;
                }

                drop(project_guard);
                state_guard.network.notify_dirty();
                log::info!("Server: Executed {}", func_name);
                return Ok(());
            }

            // サーバー側のプラグインローダーから該当するプラグインを探す
            let plugin_id = state_guard
                .common
                .plugin_loader
                .read()
                .expect("lock poisoned")
                .find_plugin_for_toolbar_button(button_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("Plugin not found for toolbar button: {}", button_id)
                })?;

            // PluginActionContextを作成
            let project = state_guard.project.clone();

            let context = PluginActionContext::new(project, *timeline_id);

            // サーバー側でスクリプトを実行
            state_guard
                .common
                .plugin_loader
                .read()
                .expect("lock poisoned")
                .call_script_with_context::<()>(&plugin_id, func_name, context)
                .map_err(|e| anyhow::anyhow!("Failed to execute toolbar script: {}", e))?;

            log::info!(
                "Server: Executed toolbar action {} from plugin {}",
                func_name,
                plugin_id
            );
        }
    }
    Ok(())
}
