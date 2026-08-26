use crate::network::ServerNetworkHandler;
use esotereel_lib::{
    ServerState,
    project::{
        change::ChangeSet,
        ids::{ClipId, LayerId, TimelineId},
        layer::LayerMeta,
    },
    responces::Response,
    util::result::{EsotereelError, LockExt},
};
use std::sync::{Arc, Mutex};

pub mod network;
pub mod project;
pub mod requests;

pub type OnServerReadyFn = extern "C" fn(bool); // 起動成功したか

pub async fn server_network_start(addr: &str, on_server_ready: Option<OnServerReadyFn>) {
    let state = ServerState::new();
    let dirty_signal = state.dirty_signal.clone();

    let network = Arc::new(ServerNetworkHandler::new(Arc::new(Mutex::new(state))));

    // async タスク用に Clone
    let network_clone = network.clone();

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
    let mut lock = app_state.project.write_or_err()?;
    let project = lock.as_mut().ok_or(EsotereelError::InvalidTimeline)?;

    let mut changes = project.drain_changes();
    if changes.is_empty() {
        return Ok(());
    }

    // Composite/Area/Script経由でネストしているclipへの波及も回収
    project.propagate_nested_dirty(&changes);
    changes.extend(project.drain_changes());

    // ロックを解放してからネットワーク送信を行う（デッドロック回避）
    drop(lock);
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
    let lock = app_state.project.read_or_err()?;
    let project = lock.as_ref().ok_or(EsotereelError::InvalidTimeline)?;
    let timeline = project
        .timeline(timeline_id)
        .ok_or(EsotereelError::InvalidTimeline)?;

    if !changeset.clips_upserted.is_empty() {
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

        if !clips.is_empty() {
            let targets = network.clients_watching_in(timeline_id, &range);
            if !targets.is_empty() {
                network.send_to_many(&targets, &Response::UpdateClip { timeline_id, clips });
            }
        }
    }

    if !changeset.clips_removed.is_empty() {
        let range = changeset
            .clips_removed
            .values()
            .fold(i64::MAX..i64::MIN, |mut r, info| {
                r.start = r.start.min(info.position);
                r.end = r.end.max(info.position + info.duration);
                r
            });

        let targets = network.clients_watching_in(timeline_id, &range);
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
        let layers: Vec<LayerMeta> = changeset
            .layers_upserted
            .iter()
            .filter_map(|id| timeline.get_layer(*id).map(LayerMeta::from))
            .collect();

        let root_layers = changeset
            .root_layers_changed
            .then(|| timeline.root_layers().to_vec());

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
            let layer_ids: Vec<LayerId> = changeset.layers_removed.iter().copied().collect();
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
