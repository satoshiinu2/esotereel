use crate::network::ServerNetworkHandler;
use esotereel_lib::{
    HostRole, ServerState,
    dirs::Directories,
    plugin::PluginLoader,
    project::{
        change::ChangeSet,
        ids::{ClipId, LayerId, TimelineId},
        layer::LayerMeta,
    },
    responces::Response,
    util::result::EsotereelError,
};
use std::sync::{Arc, Mutex};

pub mod network;
pub mod project;
pub mod requests;

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

    let network = Arc::new(ServerNetworkHandler::new(Arc::new(Mutex::new(state))));

    // async タスク用に Clone
    let network_clone = network.clone();
    let dirty_signal = network.dirty_signal.clone();

    tokio::spawn(async move {
        loop {
            dirty_signal.notified().await;
            if let Err(e) = on_project_event(&network_clone).await {
                log::error!("Handler Error: {:?}", e);
            }
        }
    });

    if let Err(e) = network.run(addr, on_server_ready).await {
        log::error!("Server failed to start: {}", e);
    }
}
async fn on_project_event(network: &Arc<ServerNetworkHandler>) -> anyhow::Result<()> {
    // Dirtyシグナルの中身は見ない。原因(自分のCommand/他ユーザー/スクリプト)を
    // 問わず、実際にProjectに溜まった差分だけを見て動く。
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let project_arc = app_state.project.as_ref().cloned();
    let Some(project_arc) = project_arc else {
        anyhow::bail!(EsotereelError::ProjectNotFound)
    };

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
    drop(app_state);

    for (timeline_id, changeset) in changes {
        dispatch_changeset(network, timeline_id, changeset)?;
    }

    Ok(())
}

fn dispatch_changeset(
    network: &Arc<ServerNetworkHandler>,
    timeline_id: TimelineId,
    changeset: ChangeSet,
) -> anyhow::Result<()> {
    // ロックを再度取得してタイムラインデータを取得
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let project_arc = app_state.project.as_ref().cloned();
    let Some(project_arc) = project_arc else {
        anyhow::bail!(EsotereelError::ProjectNotFound)
    };

    let project = project_arc.write().unwrap();

    let timeline = project
        .timeline(timeline_id)
        .ok_or(EsotereelError::InvalidTimeline)?;

    // clips_upsertedの処理
    let (range, clips) = if !changeset.clips_upserted.is_empty() {
        let (range, clips) = changeset.clips_upserted.iter().fold(
            (i64::MAX..i64::MIN, Vec::new()),
            |(mut range, mut clips), id| {
                if let Some((clip, layer_id)) = timeline.find_clip_by_id(*id) {
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

    let root_layers = changeset
        .root_layers_changed
        .then(|| timeline.root_layers().to_vec());

    let layer_ids: Vec<LayerId> = if !changeset.layers_removed.is_empty() {
        changeset.layers_removed.iter().copied().collect()
    } else {
        Vec::new()
    };

    // ロックを解放してからネットワーク送信を行う（デッドロック回避）
    drop(app_state);

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
        if !layers.is_empty() || root_layers.is_some() {
            let targets = network.clients_watching_timeline(timeline_id); // range不要、timeline全体購読者
            network.send_to_many(
                &targets,
                &Response::UpdateLayer {
                    timeline_id,
                    layers,
                    root_layers,
                },
            );
        }

        if !changeset.layers_removed.is_empty() {
            let targets = network.clients_watching_timeline(timeline_id);
            network.send_to_many(
                &targets,
                &Response::RemoveLayer {
                    timeline_id,
                    layer_ids,
                },
            );
        }
    }

    Ok(())
}
