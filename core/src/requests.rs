use std::{
    ops::Range,
    sync::{Arc, Mutex, RwLock},
};

use esotereel_lib::{
    StreamState,
    decode::videostreamer::VideoStreamer,
    plugin::{
        NamespacedID, PluginLoader, property::ComponentCapability, script::api::PluginActionContext,
    },
    project::{
        Project,
        command::{CommandHistory, CommandRequest},
        ids::TimelineId,
    },
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

            // State ロック中は project の差し替えと履歴クリアだけを行い、
            // ビュー更新と送信はロック解放後に行う（lock-order.md）
            let network = {
                let mut state = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

                state.project = Arc::new(RwLock::new(Some(new_project)));
                state
                    .histories
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?
                    .clear();

                Arc::clone(&state.network)
            };

            // クライアントのビューを初期化：すべてのタイムラインを見ているとみなす
            for timeline in &timelines {
                network.update_client_view(client_id, timeline.id, i64::MIN..i64::MAX);
            }

            let cmd = Response::ProjectMeta { timelines };
            network.send(client_id, &cmd);
        }
        ArchivedRequest::ProjectAll => {
            // 読み取り専用なので write ではなく read ロックでよい。
            // State は Arc の取り出しだけ、送信は全ガード解放後に行う。
            let (project_arc, network) = {
                let state_guard = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
                (
                    Arc::clone(&state_guard.project),
                    Arc::clone(&state_guard.network),
                )
            };

            let timelines = {
                let project_guard = project_arc.read_or_err()?;
                let project = project_guard
                    .as_ref()
                    .ok_or(EsotereelError::ProjectNotFound)?;
                project.timelines_meta()
            };

            // クライアントのビューを初期化：すべてのタイムラインを見ているとみなす
            for timeline in &timelines {
                network.update_client_view(client_id, timeline.id, i64::MIN..i64::MAX);
            }

            let cmd = Response::ProjectMeta { timelines };
            network.send(client_id, &cmd);
        }
        ArchivedRequest::Command { commands } => {
            // 必要な Arc を取り出したら State ロックは即座に解放し、
            // コマンド実行中は State を保持しない（lock-order.md）
            let (project_arc, network, histories_arc, loader_arc) = {
                let state_guard = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
                (
                    Arc::clone(&state_guard.project),
                    Arc::clone(&state_guard.network),
                    Arc::clone(&state_guard.histories),
                    Arc::clone(&state_guard.common.plugin_loader),
                )
            };
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let loader = loader_arc.read_or_err()?;

            // 拒否レスポンスはその場で送らず、全ガード解放後にまとめて送る
            let mut pending_rejections: Vec<Response> = Vec::new();

            for (timeline_id, command) in commands.iter() {
                let command: CommandRequest = command.deserialize(&mut rkyv::Infallible).unwrap();

                log::debug!(target:"core_packet_request","request: {:?}", command);

                // コンポーネント定義から能力ベースでメディアオフセットメンバーのキーを解決する。
                // プラグインが宣言する `media-offset-provider` 能力からメンバー名を逆引きし、
                // ハードコードされたプロパティキーに依存しない（AGENTS.md 第9条）。
                let offset_member_key = match &command {
                    CommandRequest::ClipsResize { clips } if !clips.is_empty() => {
                        let clip = project
                            .timeline_ref(*timeline_id)
                            .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?
                            .get_clip(clips[0].clip_id)
                            .ok_or(EsotereelError::ClipNotFound(clips[0].clip_id))?;
                        let runtime_def = loader.resolve_component_definition(&clip.kind_id)?;
                        let comp = runtime_def
                            .components
                            .iter()
                            .find(|c| c.id == clip.kind_id)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "component definition not found for kind {}",
                                    clip.kind_id
                                )
                            })?;
                        comp.member_key_by_capability(&ComponentCapability::MediaOffsetProvider)
                            .map(|k| k.clone())
                    }
                    _ => None,
                };

                match command_to_history(project, *timeline_id, command.clone()) {
                    Ok(command_history) => {
                        let mut histories = histories_arc.lock().map_err(|e| {
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
                            offset_member_key.as_deref(),
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
                        pending_rejections.extend(rejections);
                    }
                }
            }

            drop(loader);
            drop(project_guard);

            for resp in &pending_rejections {
                network.send(client_id, resp);
            }
            network.notify_dirty();
        }
        ArchivedRequest::InitStream { path } => {
            let path = path.as_ref();

            let streamer = VideoStreamer::new(path).map_err(|e| {
                EsotereelError::IoError(format!("Failed to open video stream: {:?}", e))
            })?;

            // 登録までは State ロックが必要だが、送信は解放後に行う
            let (network, res, resource_id) = {
                let mut state = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

                let resource_id = state.get_or_create_resource_id(path);

                let res = streamer.get_init_packet(path, resource_id);

                state.streams.insert(resource_id, streamer);
                state
                    .stream_state_map
                    .insert(path.to_owned(), StreamState::Loaded(resource_id));

                (Arc::clone(&state.network), res, resource_id)
            };

            network.send(client_id, &res);

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

            let (network, to_send) = {
                let state_guard = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

                let mut streamer = state_guard
                    .streams
                    .get_mut(resource_id)
                    .ok_or_else(|| EsotereelError::StreamNotFound(*resource_id))?;

                let generation = streamer.next_generation();

                let to_send = streamer.fetch_stream_data_batch(*resource_id, ranges, generation)?;
                (Arc::clone(&state_guard.network), to_send)
            };

            for res in &to_send {
                network.send(client_id, res);
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
            let range: Range<i64> = range.deserialize(&mut rkyv::Infallible).unwrap();

            // State は Arc の取り出しだけ。query_range は読み取り専用なので
            // Project は read ロックでよい。送信は全ガード解放後に行う。
            let (project_arc, network) = {
                let state_guard = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
                (
                    Arc::clone(&state_guard.project),
                    Arc::clone(&state_guard.network),
                )
            };

            // クライアントの表示範囲をサーバーに記憶させる
            network.update_client_view(client_id, *timeline_key, range.clone());

            let clips = {
                let project_guard = project_arc.read_or_err()?;
                let project = project_guard
                    .as_ref()
                    .ok_or(EsotereelError::ProjectNotFound)?;

                let timeline = project
                    .timeline_ref(*timeline_key)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_key))?;

                timeline
                    .query_range(range)
                    .into_iter()
                    .map(|(layer, clip)| (layer.id, clip.clone()))
                    .collect::<Vec<_>>()
            };

            network.send(
                client_id,
                &Response::UpdateClip {
                    timeline_id: *timeline_key,
                    clips,
                },
            );
        }
        ArchivedRequest::DebugFetchProjectStruct => {
            // 読み取り専用なので read ロックでよい。送信は解放後に行う。
            let (project_arc, network) = {
                let state_guard = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
                (
                    Arc::clone(&state_guard.project),
                    Arc::clone(&state_guard.network),
                )
            };

            let str = {
                let project_guard = project_arc.read_or_err()?;
                let project = project_guard
                    .as_ref()
                    .ok_or(EsotereelError::ProjectNotFound)?;
                format!("{:#?}", project)
            };

            network.send(client_id, &Response::DebugProjectStruct(None));
            network.send(client_id, &&Response::DebugProjectStruct(Some(str)));
        }
        ArchivedRequest::ToolbarAction {
            button_id,
            func_name,
            run_on: _,
            timeline_id,
        } => {
            let button_id = NamespacedID::parse(button_id.as_ref())?;
            let func_name = func_name.as_ref();

            let plugin_id = button_id.plugin_id();

            // Phase 1: State ロック中は必要な Arc の取り出しと、実行する
            // スクリプトのクローンだけを行う（lock-order.md）
            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

            let network = Arc::clone(&state_guard.network);
            let project_arc = Arc::clone(&state_guard.project);
            let histories_arc = Arc::clone(&state_guard.histories);
            let loader_arc = Arc::clone(&state_guard.common.plugin_loader);

            let context = PluginActionContext::new(
                Arc::clone(&network),
                Arc::clone(&project_arc),
                *timeline_id,
            );

            // スクリプトはクローンして所有し、PluginLoader の read ガードは
            // 実行前に drop する。スクリプトは set_clip_property 経由で
            // Project の write ロックを取りうるため、Loader ロック保持中の
            // 実行は禁止（lock-order.md）。
            let script = {
                let loader = loader_arc.read_or_err()?;
                loader
                    .get_script(&plugin_id)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_id))?
            }; // loader ガードはここで drop される

            // 注意: script_engine は CommonState が直接所有する値（Arc 化
            // されていない）ため、エンジン参照が要る間は State ロックを
            // 解放できない。パケット処理は解放後に回す。
            script
                .call::<()>(
                    &state_guard.common.script_engine,
                    func_name,
                    (context.clone(),),
                )
                .map_err(|e| anyhow::anyhow!("Failed to execute toolbar script: {}", e))?;

            // スクリプト内で積まれた Request を回収して処理する。
            // ここからは State ロックなしで、クローンした Arc だけで処理する。
            let packets = context.take_packets();
            drop(state_guard);

            for packet in packets {
                match packet {
                    Request::Undo { timeline_id } => {
                        let mut project_guard = project_arc.write_or_err()?;
                        let project = project_guard
                            .as_mut()
                            .ok_or(EsotereelError::ProjectNotFound)?;

                        let loader = loader_arc.read_or_err()?;

                        let mut histories = histories_arc.lock().map_err(|e| {
                            anyhow::Error::from(EsotereelError::LockError(e.to_string()))
                        })?;
                        let history = histories
                            .entry(timeline_id)
                            .or_insert_with(|| HistoryStack::default());

                        let offset_member_key = history.last_undo().and_then(|cmd| {
                            resolve_offset_member_key(project, timeline_id, cmd, &loader)
                        });
                        undo_command(project, timeline_id, history, offset_member_key.as_deref())?;
                    }
                    Request::Redo { timeline_id } => {
                        let mut project_guard = project_arc.write_or_err()?;
                        let project = project_guard
                            .as_mut()
                            .ok_or(EsotereelError::ProjectNotFound)?;

                        let loader = loader_arc.read_or_err()?;

                        let mut histories = histories_arc.lock().map_err(|e| {
                            anyhow::Error::from(EsotereelError::LockError(e.to_string()))
                        })?;
                        let history = histories
                            .entry(timeline_id)
                            .or_insert_with(|| HistoryStack::default());

                        let offset_member_key = history.last_redo().and_then(|cmd| {
                            resolve_offset_member_key(project, timeline_id, cmd, &loader)
                        });
                        redo_command(project, timeline_id, history, offset_member_key.as_deref())?;
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
            // State は Arc の取り出しだけ。実行は解放後に行う。
            let (project_arc, network, histories_arc, loader_arc) = {
                let state_guard = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
                (
                    Arc::clone(&state_guard.project),
                    Arc::clone(&state_guard.network),
                    Arc::clone(&state_guard.histories),
                    Arc::clone(&state_guard.common.plugin_loader),
                )
            };
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let loader = loader_arc.read_or_err()?;

            let mut histories = histories_arc
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            let history = histories
                .entry(*timeline_id)
                .or_insert_with(|| HistoryStack::default());

            // 履歴の最後のコマンドから能力ベースでオフセットメンバキーを解決する。
            let offset_member_key = history
                .last_undo()
                .and_then(|cmd| resolve_offset_member_key(project, *timeline_id, cmd, &loader));

            undo_command(project, *timeline_id, history, offset_member_key.as_deref())?;
            drop(project_guard);
            // must send to client
            network.notify_dirty();
        }
        ArchivedRequest::Redo { timeline_id } => {
            // State は Arc の取り出しだけ。実行は解放後に行う。
            let (project_arc, network, histories_arc, loader_arc) = {
                let state_guard = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
                (
                    Arc::clone(&state_guard.project),
                    Arc::clone(&state_guard.network),
                    Arc::clone(&state_guard.histories),
                    Arc::clone(&state_guard.common.plugin_loader),
                )
            };
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let loader = loader_arc.read_or_err()?;

            let mut histories = histories_arc
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            let history = histories
                .entry(*timeline_id)
                .or_insert_with(|| HistoryStack::default());

            let offset_member_key = history
                .last_redo()
                .and_then(|cmd| resolve_offset_member_key(project, *timeline_id, cmd, &loader));

            redo_command(project, *timeline_id, history, offset_member_key.as_deref())?;
            drop(project_guard);
            // must send to client
            network.notify_dirty();
        }
    }
    Ok(())
}

/// コマンド履歴からメディアオフセットメンバーのキーを能力ベースで解決する。
///
/// 履歴の最初のクリップの種別からコンポーネント定義を引き、
/// `MediaOffsetProvider` 能力を持つメンバーのキーを返す。
/// プラグインが宣言する能力からプロパティを参照し、ハードコードを避ける
/// （AGENTS.md 第9条）。
fn resolve_offset_member_key(
    project: &Project,
    timeline_id: TimelineId,
    command: &CommandHistory,
    loader: &PluginLoader,
) -> Option<String> {
    match command {
        CommandHistory::ClipsResize { clips } => clips.first().and_then(|ctx| {
            let clip = project
                .timeline_ref(timeline_id)?
                .get_clip(ctx.clip_id)?;
            let runtime_def = loader.resolve_component_definition(&clip.kind_id).ok()?;
            let comp = runtime_def
                .components
                .iter()
                .find(|c| c.id == clip.kind_id)?;
            comp.member_key_by_capability(&ComponentCapability::MediaOffsetProvider)
                .cloned()
        }),
        _ => None,
    }
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
