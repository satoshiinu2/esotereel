use std::sync::Arc;

use esotereel_lib::{
    project::{
        change::ChangeSet,
        ids::{ClipId, LayerFolderId, LayerId, TimelineId},
        layer::LayerMeta,
        layer_outline::{Meta, OutlineNode},
    },
    responces::Response,
    util::result::{EsotereelError, LockExt},
};

use crate::{
    save::{broadcast_asset_updates, broadcast_kind_appends},
    state::ServerState,
};

pub async fn on_project_event(state: &Arc<ServerState>) -> anyhow::Result<()> {
    // Dirtyシグナルの中身は見ない。原因(自分のCommand/他ユーザー/スクリプト)を
    // 問わず、実際にProjectに溜まった差分だけを見て動く。
    let changes = {
        let mut project_guard = state.project.write_or_err()?;
        let project = project_guard
            .as_mut()
            .ok_or(EsotereelError::ProjectNotFound)?;

        let changes = project.drain_changes();
        if changes.is_empty() {
            return Ok(());
        }

        changes
    };

    // 変更があったのでオートセーブ対象にし、新規採番された kind が
    // あれば増分を全クライアントへ先に配信する（この後の変更通知で
    // 追加されたクリップの配置が連番を参照するため）。
    state
        .project_dirty
        .store(true, std::sync::atomic::Ordering::SeqCst);

    broadcast_kind_appends(state);
    // クリップ追加に伴うアセット登録など、レジストリ側の変更も
    // 差分があれば全クライアントへ配る。
    broadcast_asset_updates(state);

    // Project ロックを解放してからネットワーク送信を行う（デッドロック回避）
    // 1タイムラインの失敗で残りの変更まで捨てないよう、全部を配送しきってから
    // 最初のエラーを返す。
    let mut first_error: Option<anyhow::Error> = None;
    for (timeline_id, changeset) in changes {
        if let Err(error) = dispatch_changeset(state, timeline_id, changeset) {
            log::error!("failed to dispatch changeset for timeline {timeline_id}: {error:#}");
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn dispatch_changeset(
    state: &Arc<ServerState>,
    timeline_id: TimelineId,
    changeset: ChangeSet,
) -> anyhow::Result<()> {
    // タイムラインデータを取得
    let network = Arc::clone(&state.network);
    let mut project_guard = state.project.write_or_err()?;
    let project = project_guard
        .as_mut()
        .ok_or(EsotereelError::ProjectNotFound)?;

    let timeline = project
        .timeline_ref(timeline_id)
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
                        opacity: f.opacity.clone(),
                        blend_mode: f.blend_mode.clone(),
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
    drop(project_guard);

    // ネットワーク送信
    for prop_change in &changeset.clip_properties_changed {
        let targets = network.clients_watching_timeline(timeline_id);
        if !targets.is_empty() {
            network.send_to_many(
                &targets,
                &Response::UpdateClipProperty {
                    timeline_id,
                    clip_id: prop_change.clip_id,
                    key: prop_change.key.clone(),
                    value: prop_change.value.clone(),
                },
            );
        }
    }

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
