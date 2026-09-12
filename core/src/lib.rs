use crate::state::ServerState;
use esotereel_lib::{
    HostRole,
    dirs::Directories,
    plugin::PluginLoader,
    project::{
        change::ChangeSet,
        ids::{ClipId, LayerFolderId, LayerId, TimelineId},
        layer::LayerMeta,
        layer_outline::{Meta, OutlineNode},
    },
    responces::Response,
    util::result::EsotereelError,
};
use std::sync::{Arc, Mutex};

pub mod network;
pub mod project;
pub mod requests;
pub mod state;

pub async fn server_network_start<F>(
    addr: &str,
    on_server_ready: Option<F>, // 起動成功したか, アドレス

    dirs_def: Directories,
    plugin_loader: Option<Arc<Mutex<PluginLoader>>>, // クライアント側から提供
) where
    F: FnOnce(bool, &str),
{
    let was_plugin_producted = plugin_loader.is_some();
    let mut state = ServerState::new(dirs_def, plugin_loader);

    // プラグインが提供されたものではなかったらプラグインを並列で読み込む(すでに読み込まれているので)
    if !was_plugin_producted {
        if let Err(e) = state.load_plugins(HostRole::Server).await {
            log::error!("Failed to load plugins: {}", e);
        } else {
            log::info!("Server plugins loaded successfully");
        }
    }

    // async タスク用に Clone
    let dirty_signal = Arc::clone(&state.dirty_signal);
    let network_clone = Arc::clone(&state.network);

    let state_arc = Arc::new(Mutex::new(state));
    let state_clone = Arc::clone(&state_arc);

    tokio::spawn(async move {
        loop {
            dirty_signal.notified().await;
            if let Err(e) = on_project_event(&state_clone).await {
                log::error!("Handler Error: {:?}", e);
            }
        }
    });

    let state_clone = Arc::clone(&state_arc);
    if let Err(e) = network_clone.run(state_clone, addr, on_server_ready).await {
        log::error!("Server failed to start: {}", e);
    }
}
async fn on_project_event(state: &Arc<Mutex<ServerState>>) -> anyhow::Result<()> {
    // Dirtyシグナルの中身は見ない。原因(自分のCommand/他ユーザー/スクリプト)を
    // 問わず、実際にProjectに溜まった差分だけを見て動く。
    let state_lock = state.lock().expect("mutex poisoned");

    let project_arc = state_lock.project.as_ref();
    let project_arc = project_arc.ok_or_else(|| EsotereelError::ProjectNotFound)?;
    let project_arc = Arc::new(project_arc);

    let changes = {
        let mut project = project_arc.write().unwrap();

        let mut changes = project.drain_changes();
        if changes.is_empty() {
            return Ok(());
        }

        // Composite/Area/Script経由でネストしているclipへの波及も回収
        project.propagate_nested_dirty(&changes);
        changes.extend(project.drain_changes());

        changes
    };

    // ロックを解放してからネットワーク送信を行う（デッドロック回避）
    drop(state_lock);

    for (timeline_id, changeset) in changes {
        dispatch_changeset(state, timeline_id, changeset)?;
    }

    Ok(())
}

fn dispatch_changeset(
    state: &Arc<Mutex<ServerState>>,
    timeline_id: TimelineId,
    changeset: ChangeSet,
) -> anyhow::Result<()> {
    // ロックを再度取得してタイムラインデータを取得
    let state_lock = state.lock().expect("mutex poisoned");
    let network = Arc::clone(&state_lock.network);

    let project_arc = state_lock.project.as_ref();
    let project_arc = project_arc.ok_or_else(|| EsotereelError::ProjectNotFound)?;
    let project_arc = Arc::clone(&project_arc);

    let project = project_arc.write().unwrap();

    let timeline = project
        .timeline(timeline_id)
        .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

    // clips_upsertedの処理
    let (range, clips) = if !changeset.clips_upserted.is_empty() {
        let (range, clips) = changeset.clips_upserted.iter().fold(
            (i64::MAX..i64::MIN, Vec::new()),
            |(mut range, mut clips), id| {
                if let Some((clip, layer_id)) = timeline.get_clip_and_layer(*id) {
                    range.start = range.start.min(clip.position);
                    range.end = range.end.max(clip.position + clip.duration);
                    clips.push((layer_id, clip.clone()));
                }
                (range, clips)
            },
        );
        (range, clips)
    } else {
        (i64::MAX..i64::MIN, Vec::new())
    };

    // clips_removedの処理
    let removed_range = if !changeset.clips_removed.is_empty() {
        let range = changeset
            .clips_removed
            .values()
            .fold(i64::MAX..i64::MIN, |mut r, info| {
                r.start = r.start.min(info.position);
                r.end = r.end.max(info.position + info.duration);
                r
            });
        range
    } else {
        i64::MAX..i64::MIN
    };

    // layersの処理
    let layers: Vec<LayerMeta> = if !changeset.is_layer_empty() {
        changeset
            .layers_upserted
            .iter()
            .filter_map(|id| timeline.get_layer(*id).map(LayerMeta::from))
            .collect()
    } else {
        Vec::new()
    };

    let removed_layer_ids: Vec<LayerId> = if !changeset.layers_removed.is_empty() {
        changeset.layers_removed.iter().copied().collect()
    } else {
        Vec::new()
    };

    let outline_folders: Vec<(LayerFolderId, Meta)> = changeset
        .outline_folders_upserted
        .iter()
        .filter_map(|id| {
            timeline.get_folder(*id).map(|f| {
                (
                    *id,
                    Meta {
                        name: f.name.clone(),
                    },
                )
            })
        })
        .collect();

    let outline_children: Vec<(Option<LayerFolderId>, Vec<OutlineNode>)> = changeset
        .outline_children_changed
        .iter()
        .map(|parent| (*parent, timeline.outline.children_of(*parent).to_vec()))
        .collect();

    let outline_folders_removed: Vec<LayerFolderId> =
        changeset.outline_folders_removed.iter().copied().collect();

    // ロックを解放してからネットワーク送信を行う（デッドロック回避）
    drop(state_lock);

    // ネットワーク送信
    if !clips.is_empty() {
        let targets = network.clients_watching_in(timeline_id, &range);
        if !targets.is_empty() {
            network.send_to_many(&targets, &Response::UpdateClip { timeline_id, clips });
        }
    }

    if !changeset.clips_removed.is_empty() {
        let targets = network.clients_watching_in(timeline_id, &removed_range);
        if !targets.is_empty() {
            let clip_ids: Vec<(LayerId, ClipId)> = changeset
                .clips_removed
                .iter()
                .map(|(&id, info)| (info.layer_id, id))
                .collect();
            network.send_to_many(
                &targets,
                &Response::RemoveClip {
                    timeline_id,
                    clip_ids,
                },
            );
        }
    }

    if !changeset.is_layer_empty() {
        if !layers.is_empty() {
            let targets = network.clients_watching_timeline(timeline_id); // range不要、timeline全体購読者
            network.send_to_many(
                &targets,
                &Response::UpdateLayer {
                    timeline_id,
                    layers,
                },
            );
        }

        if !changeset.layers_removed.is_empty() {
            let targets = network.clients_watching_timeline(timeline_id);
            network.send_to_many(
                &targets,
                &Response::RemoveLayer {
                    timeline_id,
                    layer_ids: removed_layer_ids,
                },
            );
        }
    }

    if !outline_folders.is_empty() || !outline_children.is_empty() {
        let targets = network.clients_watching_timeline(timeline_id); // layersと同じくrange不要
        network.send_to_many(
            &targets,
            &Response::UpdateOutline {
                timeline_id,
                folders: outline_folders,
                children: outline_children,
            },
        );
    }

    if !outline_folders_removed.is_empty() {
        let targets = network.clients_watching_timeline(timeline_id);
        network.send_to_many(
            &targets,
            &Response::Removes {
                timeline_id,
                folder_ids: outline_folders_removed,
            },
        );
    }

    Ok(())
}
