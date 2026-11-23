use std::{
    ops::Range,
    sync::{Arc, RwLock},
};

use esotereel_lib::{
    decode::videostreamer::VideoStreamer,
    plugin::{
        clip::component::Scheduler, script::api::PluginActionContext, NamespacedID, PluginLoader,
    },
    project::{
        command::{CommandHistory, CommandRequest},
        ids::TimelineId,
        Project,
    },
    requests::{AddClipFromAsset, ArchivedAddClipFromAsset, ArchivedRequest, Request},
    responces::Response,
    util::result::{EsotereelError, EsotereelResult, LockExt, MutexExt},
    StreamState,
};
use rkyv::Deserialize;

use uuid::Uuid;

use crate::{
    network::ServerNetworkHandler,
    project::{
        clip_add_core,
        commands::command_to_history,
        history::{execute_command_with_history, redo_command, undo_command, HistoryStack},
        MediaPropertyPaths,
    },
    state::ServerState,
};

pub fn on_request_receive(
    request: &ArchivedRequest,
    client_id: u32,
    state: &ServerState,
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

            // project の差し替えと履歴クリアは Project の write ロック
            // 保持中に一体で行い、ビュー更新と送信は解放後に行う
            // （lock-order.md）。差し替えは Arc の付け替えではなく既存の
            // RwLock へのインプレース書き込みで行う。
            let network = {
                let mut project_guard = state.project.write_or_err()?;
                state.histories.lock_or_err()?.clear();
                *project_guard = Some(new_project);

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
            // 送信は Project ロック解放後に行う。
            let (project_arc, network) = (Arc::clone(&state.project), Arc::clone(&state.network));

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
        ArchivedRequest::RegisterAssetReference {
            asset_id,
            path,
            add_clip,
        } => {
            // 必要な Arc はフィールドから直接取り出す（lock-order.md）
            let (project_arc, network, loader_arc, script_engine) = (
                Arc::clone(&state.project),
                Arc::clone(&state.network),
                Arc::clone(&state.common.plugin_loader),
                Arc::clone(&state.common.script_engine),
            );
            let asset_id_raw: &str = asset_id;
            let path_str: &str = path;
            let result = (|| -> anyhow::Result<bool> {
                let asset_uuid = parse_asset_id(asset_id_raw)?;
                let source_path = std::path::Path::new(path_str);
                if !source_path.is_file() {
                    anyhow::bail!("asset source `{path_str}` is not an existing file");
                }
                let add_clip = deserialize_add_clip(add_clip)?;
                let mut project_guard = project_arc.write_or_err()?;
                let project = project_guard
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("no active project"))?;
                project.register_external_asset(asset_uuid, source_path)?;
                let mut clip_added = false;
                if let Some(add_clip) = add_clip {
                    add_clip_from_asset(project, &loader_arc, &script_engine, &add_clip, path_str)?;
                    clip_added = true;
                }
                Ok(clip_added)
            })();
            match result {
                Ok(clip_added) => {
                    log::info!("asset reference registered: {asset_id_raw} -> {path_str}");
                    network.send(
                        client_id,
                        &Response::AssetImported {
                            asset_id: asset_id_raw.to_string(),
                        },
                    );
                    if clip_added {
                        network.notify_dirty();
                    }
                }
                Err(error) => send_asset_import_failed(&network, client_id, asset_id_raw, &error),
            }
        }
        ArchivedRequest::BeginAssetUpload {
            asset_id,
            file_name,
            total_size,
        } => {
            let (network, uploads) = (Arc::clone(&state.network), Arc::clone(&state.uploads));
            let asset_id_raw: &str = asset_id;
            let file_name_str: &str = file_name;
            let result = (|| -> anyhow::Result<()> {
                let asset_uuid = parse_asset_id(asset_id_raw)?;
                let mut uploads = uploads.lock_or_err()?;
                uploads.begin(asset_uuid, file_name_str, *total_size)
            })();
            if let Err(error) = result {
                send_asset_import_failed(&network, client_id, asset_id_raw, &error);
            }
        }
        ArchivedRequest::AssetUploadChunk {
            asset_id,
            offset,
            data,
        } => {
            let (network, uploads) = (Arc::clone(&state.network), Arc::clone(&state.uploads));
            let asset_id_raw: &str = asset_id;
            let result = (|| -> anyhow::Result<()> {
                let asset_uuid = parse_asset_id(asset_id_raw)?;
                let mut uploads = uploads.lock_or_err()?;
                uploads.write_chunk(asset_uuid, *offset, data)
            })();
            if let Err(error) = result {
                send_asset_import_failed(&network, client_id, asset_id_raw, &error);
            }
        }
        ArchivedRequest::FinishAssetUpload { asset_id, add_clip } => {
            // 必要な Arc はフィールドから直接取り出す（lock-order.md）
            let (project_arc, network, uploads, loader_arc, script_engine) = (
                Arc::clone(&state.project),
                Arc::clone(&state.network),
                Arc::clone(&state.uploads),
                Arc::clone(&state.common.plugin_loader),
                Arc::clone(&state.common.script_engine),
            );
            let asset_id_raw: &str = asset_id;
            let result = (|| -> anyhow::Result<bool> {
                let asset_uuid = parse_asset_id(asset_id_raw)?;
                let add_clip = deserialize_add_clip(add_clip)?;
                // 先にアップロード側で完了を検証してセッションを閉じ、
                // その後でプロジェクトの台帳へ登録する（ロックの保持範囲を分ける）。
                let (name, path) = {
                    let mut uploads = uploads.lock_or_err()?;
                    uploads.complete(asset_uuid)?
                };
                let mut project_guard = project_arc.write_or_err()?;
                let project = project_guard
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("no active project"))?;
                project.register_managed_asset(asset_uuid, name, path.clone())?;
                let mut clip_added = false;
                if let Some(add_clip) = add_clip {
                    let source = path
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("asset path is not valid UTF-8"))?;
                    add_clip_from_asset(project, &loader_arc, &script_engine, &add_clip, source)?;
                    clip_added = true;
                }
                Ok(clip_added)
            })();
            match result {
                Ok(clip_added) => {
                    network.send(
                        client_id,
                        &Response::AssetImported {
                            asset_id: asset_id_raw.to_string(),
                        },
                    );
                    if clip_added {
                        network.notify_dirty();
                    }
                }
                Err(error) => send_asset_import_failed(&network, client_id, asset_id_raw, &error),
            }
        }
        ArchivedRequest::Command { commands } => {
            // コマンド実行は取り出した Arc だけで行う（lock-order.md）
            let (project_arc, network, histories_arc, loader_arc, script_engine) = (
                Arc::clone(&state.project),
                Arc::clone(&state.network),
                Arc::clone(&state.histories),
                Arc::clone(&state.common.plugin_loader),
                Arc::clone(&state.common.script_engine),
            );
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let loader = loader_arc.read_or_err()?;

            // 拒否レスポンスはその場で送らず、全ガード解放後にまとめて送る
            let mut pending_rejections: Vec<Response> = Vec::new();
            let mut pending_file_drop_failures: Vec<Response> = Vec::new();

            for (timeline_id, command) in commands.iter() {
                let command: CommandRequest = command.deserialize(&mut rkyv::Infallible).unwrap();
                let command = match command {
                    CommandRequest::AddClipFromFile {
                        layer_id,
                        position,
                        duration,
                        kind_id,
                        source_path,
                    } => {
                        let properties = match Scheduler::new().file_drop_properties(
                            &script_engine,
                            &loader,
                            &kind_id,
                            &source_path,
                        ) {
                            Ok(properties) => properties,
                            Err(error) => {
                                log::error!("File drop handler failed: {error:#}");
                                pending_file_drop_failures.push(Response::Failed {
                                    error: EsotereelError::FileDropFailed(format!(
                                        "file drop `{source_path}`: {error:#}"
                                    )),
                                });
                                continue;
                            }
                        };
                        CommandRequest::AddClip {
                            layer_id,
                            position,
                            duration,
                            kind_id,
                            properties,
                        }
                    }
                    command => command,
                };

                log::debug!(target:"core_packet_request","request: {:?}", command);

                // コンポーネント定義から能力ベースでメディアプロパティのパスを解決する。
                // プラグインが宣言する能力（`frame-provider` とそのメンバー能力）から
                // 逆引きし、ハードコードされたプロパティキーに依存しない（AGENTS.md 第9条）。
                let media_paths = match &command {
                    CommandRequest::ClipsResize { clips } if !clips.is_empty() => {
                        let clip = project
                            .timeline_ref(*timeline_id)
                            .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?
                            .get_clip(clips[0].clip_id)
                            .ok_or(EsotereelError::ClipNotFound(clips[0].clip_id))?;
                        let runtime_def = loader.resolve_component_definition(&clip.kind_id)?;
                        Some(MediaPropertyPaths::resolve(&runtime_def))
                    }
                    CommandRequest::AddClip { kind_id, .. } => loader
                        .resolve_component_definition(kind_id)
                        .map(|def| MediaPropertyPaths::resolve(&def))
                        .ok(),
                    _ => None,
                };

                match command_to_history(project, *timeline_id, command.clone()) {
                    Ok(command_history) => {
                        let mut histories = histories_arc.lock_or_err()?;
                        let history = histories
                            .entry(*timeline_id)
                            .or_insert_with(|| HistoryStack::default());
                        if let Err(e) = execute_command_with_history(
                            project,
                            *timeline_id,
                            command_history,
                            history,
                            media_paths.as_ref(),
                        ) {
                            log::error!("Command execution error: {:?}", e);
                        } else {
                            log::debug!(target:"core_packet_history","history: {:?}", history.last_undo());
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
            for resp in &pending_file_drop_failures {
                network.send(client_id, resp);
            }
            network.notify_dirty();
        }
        ArchivedRequest::InitStream { path } => {
            let path = path.as_ref();

            let streamer = VideoStreamer::new(path).map_err(|e| {
                EsotereelError::IoError(format!("Failed to open video stream: {:?}", e))
            })?;

            // 登録は各フィールドの内部同期（DashMap / Atomic）で行い、
            // 送信はその後に行う
            let (network, res, resource_id) = {
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
                let mut streamer = state
                    .streams
                    .get_mut(resource_id)
                    .ok_or_else(|| EsotereelError::StreamNotFound(*resource_id))?;

                let generation = streamer.next_generation();

                let to_send = streamer.fetch_stream_data_batch(*resource_id, ranges, generation)?;
                (Arc::clone(&state.network), to_send)
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

            // query_range は読み取り専用なので
            // Project は read ロックでよい。送信は全ガード解放後に行う。
            let (project_arc, network) = (Arc::clone(&state.project), Arc::clone(&state.network));

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
            let (project_arc, network) = (Arc::clone(&state.project), Arc::clone(&state.network));

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

            // Phase 1: 必要な Arc の取り出しと、実行するスクリプトの
            // クローンだけを先に行う（lock-order.md）
            let network = Arc::clone(&state.network);
            let project_arc = Arc::clone(&state.project);
            let histories_arc = Arc::clone(&state.histories);
            let loader_arc = Arc::clone(&state.common.plugin_loader);
            let script_engine = Arc::clone(&state.common.script_engine);

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

            // script_engine は Arc なので、クローンを取り出しておけば
            // 他のロックを保持せずにスクリプトを実行できる。
            script
                .call::<()>(&script_engine, func_name, (context.clone(),))
                .map_err(|e| anyhow::anyhow!("Failed to execute toolbar script: {}", e))?;

            // スクリプト内で積まれた Request を回収して処理する。
            // ここからはクローンした Arc だけで処理する。
            let packets = context.take_packets();

            for packet in packets {
                match packet {
                    Request::Undo { timeline_id } => {
                        let mut project_guard = project_arc.write_or_err()?;
                        let project = project_guard
                            .as_mut()
                            .ok_or(EsotereelError::ProjectNotFound)?;

                        let loader = loader_arc.read_or_err()?;

                        let mut histories = histories_arc.lock_or_err()?;
                        let history = histories
                            .entry(timeline_id)
                            .or_insert_with(|| HistoryStack::default());

                        let media_paths = history.last_undo().and_then(|cmd| {
                            resolve_media_paths(project, timeline_id, cmd, &loader)
                        });
                        undo_command(project, timeline_id, history, media_paths.as_ref())?;
                    }
                    Request::Redo { timeline_id } => {
                        let mut project_guard = project_arc.write_or_err()?;
                        let project = project_guard
                            .as_mut()
                            .ok_or(EsotereelError::ProjectNotFound)?;

                        let loader = loader_arc.read_or_err()?;

                        let mut histories = histories_arc.lock_or_err()?;
                        let history = histories
                            .entry(timeline_id)
                            .or_insert_with(|| HistoryStack::default());

                        let media_paths = history.last_redo().and_then(|cmd| {
                            resolve_media_paths(project, timeline_id, cmd, &loader)
                        });
                        redo_command(project, timeline_id, history, media_paths.as_ref())?;
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
            // 実行は取り出した Arc だけで行う。
            let (project_arc, network, histories_arc, loader_arc) = (
                Arc::clone(&state.project),
                Arc::clone(&state.network),
                Arc::clone(&state.histories),
                Arc::clone(&state.common.plugin_loader),
            );
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let loader = loader_arc.read_or_err()?;

            let mut histories = histories_arc.lock_or_err()?;
            let history = histories
                .entry(*timeline_id)
                .or_insert_with(|| HistoryStack::default());

            // 履歴の最後のコマンドから能力ベースでメディアプロパティのパスを解決する。
            let media_paths = history
                .last_undo()
                .and_then(|cmd| resolve_media_paths(project, *timeline_id, cmd, &loader));

            undo_command(project, *timeline_id, history, media_paths.as_ref())?;
            drop(project_guard);
            // must send to client
            network.notify_dirty();
        }
        ArchivedRequest::Redo { timeline_id } => {
            // 実行は取り出した Arc だけで行う。
            let (project_arc, network, histories_arc, loader_arc) = (
                Arc::clone(&state.project),
                Arc::clone(&state.network),
                Arc::clone(&state.histories),
                Arc::clone(&state.common.plugin_loader),
            );
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let loader = loader_arc.read_or_err()?;

            let mut histories = histories_arc.lock_or_err()?;
            let history = histories
                .entry(*timeline_id)
                .or_insert_with(|| HistoryStack::default());

            let media_paths = history
                .last_redo()
                .and_then(|cmd| resolve_media_paths(project, *timeline_id, cmd, &loader));

            redo_command(project, *timeline_id, history, media_paths.as_ref())?;
            drop(project_guard);
            // must send to client
            network.notify_dirty();
        }
    }
    Ok(())
}

/// コマンド履歴からメディアプロパティのパスを能力ベースで解決する。
///
/// 履歴コマンドの対象クリップ（または追加されるクリップ）の種別から
/// コンポーネント定義を引き、`FrameProvider` 能力とそのメンバー能力から
/// パスを逆引きする。プラグインが宣言する能力からプロパティを参照し、
/// ハードコードを避ける（AGENTS.md 第9条）。
fn resolve_media_paths(
    project: &Project,
    timeline_id: TimelineId,
    command: &CommandHistory,
    loader: &PluginLoader,
) -> Option<MediaPropertyPaths> {
    let kind_id = match command {
        CommandHistory::ClipsResize { clips } => {
            let ctx = clips.first()?;
            let clip = project.timeline_ref(timeline_id)?.get_clip(ctx.clip_id)?;
            clip.kind_id.clone()
        }
        CommandHistory::AddClip { kind_id, .. } => kind_id.clone(),
        _ => return None,
    };
    let runtime_def = loader.resolve_component_definition(&kind_id).ok()?;
    Some(MediaPropertyPaths::resolve(&runtime_def))
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

/// ワイヤ形式のアセット ID を `AssetId`（UUID）として解釈する。
fn parse_asset_id(asset_id: &str) -> anyhow::Result<Uuid> {
    Uuid::parse_str(asset_id).map_err(|_| anyhow::anyhow!("invalid asset id `{asset_id}`"))
}

fn deserialize_add_clip(
    add_clip: &rkyv::option::ArchivedOption<ArchivedAddClipFromAsset>,
) -> anyhow::Result<Option<AddClipFromAsset>> {
    match add_clip.as_ref() {
        None => Ok(None),
        Some(archived) => match archived.deserialize(&mut rkyv::Infallible) {
            Ok(value) => Ok(Some(value)),
            Err(error) => match error {},
        },
    }
}

/// FileDropHandler で初期プロパティを生成し、アセットを指すクリップを追加する。
fn add_clip_from_asset(
    project: &mut Project,
    loader_arc: &Arc<RwLock<PluginLoader>>,
    script_engine: &rhai::Engine,
    add_clip: &AddClipFromAsset,
    source_path: &str,
) -> anyhow::Result<()> {
    let loader_guard = loader_arc.read_or_err()?;
    let properties = Scheduler::new().file_drop_properties(
        script_engine,
        &loader_guard,
        &add_clip.kind_id,
        source_path,
    )?;
    let media_paths = loader_guard
        .resolve_component_definition(&add_clip.kind_id)
        .map(|def| MediaPropertyPaths::resolve(&def))
        .ok();
    clip_add_core(
        project,
        add_clip.timeline_id,
        add_clip.layer_id,
        add_clip.position,
        add_clip.duration,
        add_clip.kind_id.clone(),
        properties,
        media_paths.as_ref(),
        None,
    )?;
    Ok(())
}

fn send_asset_import_failed(
    network: &ServerNetworkHandler,
    client_id: u32,
    asset_id: &str,
    error: &anyhow::Error,
) {
    log::error!("asset import failed for `{asset_id}`: {error:#}");
    network.send(
        client_id,
        &Response::Failed {
            error: EsotereelError::UploadFailed(format!("asset import `{asset_id}`: {error:#}")),
        },
    );
}
