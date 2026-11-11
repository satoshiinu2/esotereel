use std::{
    ops::Range,
    path::Path,
    sync::{Arc, Mutex, RwLock},
};

use colored::Colorize;
use esotereel_lib::{
    StreamState,
    decode::streamplayer::{FetchState, StreamPlayer},
    plugin::{NamespacedID, property::value::FieldValue},
    project::{
        Project,
        clip::{Clip, ClipBindingValue},
        ids::{ClipId, LayerFolderId, LayerId, TimelineId},
        layer::LayerMeta,
        layer_outline::{Meta, OutlineNode},
        timeline::TimelineMeta,
    },
    responces::ArchivedResponse,
    util::result::{EsotereelError, EsotereelResult, LockExt},
};
use log::info;
use rkyv::Deserialize;

use crate::state::ClientState;

/// State Mutex は project の `Arc` とコールバック関数ポインタを取り出す間だけ
/// 保持し、Project ロックを取得する前に解放する。
/// ネットワークスレッドが State を保持したまま Project を待つと、Project の
/// read ガードを保持して State を取りに来る GUI スレッドと AB-BA デッドロック
/// になるため（docs/architecture/lock-order.md）。
fn project_and_callback(
    state: &Mutex<ClientState>,
) -> EsotereelResult<(Arc<RwLock<Option<Project>>>, extern "C" fn(TimelineId))> {
    let state_guard = state
        .lock()
        .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
    Ok((
        Arc::clone(&state_guard.project),
        state_guard.gui_callbacks.mark_dirty_timeline,
    ))
} // state_guard はここで drop される

/// デコード等の重い処理を State ロックなしで行うため、stream_players の
/// `Arc` だけを短時間の State ロックで取り出す。
fn stream_players_of(
    state: &Mutex<ClientState>,
) -> EsotereelResult<Arc<dashmap::DashMap<esotereel_lib::project::ids::ResourceId, StreamPlayer>>> {
    let state_guard = state
        .lock()
        .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
    Ok(Arc::clone(&state_guard.stream_players))
}

pub(super) fn on_responce_recveve(
    responce: &ArchivedResponse,
    state: &Mutex<ClientState>,
) -> EsotereelResult<()> {
    match responce {
        ArchivedResponse::Test => {}
        ArchivedResponse::ProjectMeta { timelines } => {
            let timeline_metas: Vec<TimelineMeta> = timelines
                .iter()
                .map(|archived| {
                    let layer_metas: Vec<LayerMeta> = archived
                        .layers
                        .iter()
                        .map(|al| LayerMeta {
                            id: al.id,
                            name: al.name.as_str().to_string(),
                            enabled: al.enabled,
                        })
                        .collect();

                    let outline = archived.outline.deserialize(&mut rkyv::Infallible).unwrap();

                    TimelineMeta {
                        id: archived.id,
                        fps: archived.fps,
                        layers: layer_metas,
                        outline,
                    }
                })
                .collect();
            info!("timeline_metas: {:?}", &timeline_metas);

            let real_project = Project::from_meta(timeline_metas);
            let timeline_len = real_project.timeline_count();
            let project_arc = Arc::new(RwLock::new(Some(real_project)));

            let mut state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            state_guard.project = project_arc;
            // プロジェクトが再初期化されたため、タイムラインごとの最後の
            // FetchClipsInRange 送信記録をクリアする（新しいプロジェクトでの
            // 重複送信抑制が誤って働かないようにする）
            state_guard.last_clips_fetch.clear();

            let fn_ptr = state_guard.gui_callbacks.mark_dirty_timeline;
            drop(state_guard);

            for i in 0..timeline_len {
                fn_ptr(i as TimelineId);
            }
        }
        ArchivedResponse::UpdateClip { timeline_id, clips } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                let mut sources = Vec::new();
                for (layer_id, archived_clip) in clips.iter() {
                    let clip = archived_clip.deserialize(&mut rkyv::Infallible).unwrap();
                    if let Some(source) = video_source_path(&clip) {
                        sources.push(source);
                    }
                    timeline.upsert_clip_from_network(*layer_id, clip);
                }
                // クライアント側のアセットレジストリにも動画ソースを登録し、
                // Project.assets をサーバーと整合したカタログとして保つ。
                for source in sources {
                    let _ = project.register_asset(Path::new(&source));
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }
        ArchivedResponse::UpdateClipProperty {
            timeline_id,
            clip_id,
            key,
            value,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let mut new_source = None;
            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                let clip_id: ClipId = clip_id.deserialize(&mut rkyv::Infallible).unwrap();
                let key: NamespacedID = key.deserialize(&mut rkyv::Infallible).unwrap();
                let value: ClipBindingValue = value.deserialize(&mut rkyv::Infallible).unwrap();

                // プロパティ変更が動画ソースなら、タイムライン貸し出しを解放した後に
                // クライアントのアセットレジストリにも反映する。
                new_source = video_source_from_property(&key, &value);

                if let Some(clip) = timeline.get_clip_mut(clip_id) {
                    clip.set_property_value(key, value)?;
                }
            }

            if let Some(source) = new_source {
                let _ = project.register_asset(Path::new(&source));
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }
        ArchivedResponse::RemoveClip {
            timeline_id,
            clip_ids,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                for (layer_id, clip_id) in clip_ids.iter() {
                    timeline.remove_clip_by_id_in(*layer_id, *clip_id);
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }
        ArchivedResponse::CommandRejected {
            timeline_id,
            clip_id,
            current_position,
            current_duration,
            current_layer_id: _,
            ..
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let timeline = project
                .timeline_mut(*timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

            // サーバーが通知した正しい状態でローカルのクリップを上書きし、
            // 次の操作でステールネスエラーが再発しないようにする
            if let Some(clip) = timeline.get_clip_mut(*clip_id) {
                clip.set_position(*current_position);
                clip.duration = *current_duration;
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }

        ArchivedResponse::UpdateLayer {
            timeline_id,
            layers,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                for archived_layer in layers.iter() {
                    let meta = LayerMeta {
                        id: archived_layer.id,
                        name: archived_layer.name.as_str().to_string(),
                        enabled: archived_layer.enabled,
                    };
                    timeline.apply_layer_meta(meta);
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }
        ArchivedResponse::RemoveLayer {
            timeline_id,
            layer_ids,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                for archived_id in layer_ids.iter() {
                    let id: LayerId = archived_id.deserialize(&mut rkyv::Infallible).unwrap();
                    timeline.remove_layer_local(id); // サーバー側のremove_layerと違い、
                    // parentのchildren書き換えは不要
                    // (親も別途upsertで送られてくるので)
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }

        ArchivedResponse::UpdateOutline {
            timeline_id,
            folders,
            children,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                for (id, meta) in folders.iter() {
                    let meta: Meta = meta.deserialize(&mut rkyv::Infallible).unwrap();
                    timeline.apply_outline_folder_meta(*id, meta);
                }
                for (parent, kids) in children.iter() {
                    let parent: Option<LayerFolderId> =
                        parent.deserialize(&mut rkyv::Infallible).unwrap();
                    let kids: Vec<OutlineNode> = kids.deserialize(&mut rkyv::Infallible).unwrap();
                    timeline.apply_outline_children(parent, kids);
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }

        ArchivedResponse::Removes {
            timeline_id,
            folder_ids,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;
                for archived_id in folder_ids.iter() {
                    let id: LayerFolderId = archived_id.deserialize(&mut rkyv::Infallible).unwrap();
                    timeline.remove_outline_folder_local(id);
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }

        ArchivedResponse::StreamMetadata {
            path,
            resource_id,
            codec_id,
            width,
            height,
            time_base,
            extradata,
            ..
        } => {
            let codec_id = unsafe { std::mem::transmute(*codec_id) };

            let player = StreamPlayer::new_from_metadata(
                codec_id,
                *width,
                *height,
                *time_base,
                extradata.as_slice(),
            )
            .map_err(|e| esotereel_lib::util::result::EsotereelError::IoError(e.to_string()))?;

            {
                // この State ロックは DashMap への挿入のみで、他のロックを
                // 取得しないため短時間保持のままにできる
                let state = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
                state.stream_players.insert(*resource_id, player);
                state
                    .stream_state_map
                    .insert(path.as_ref().to_owned(), StreamState::Loaded(*resource_id));
                // 新しいストリームプレイヤーが割り当てられたため、過去の
                // FetchStreamData 送信記録をクリアする（別メディアの同じ
                // resource_id に旧記録が誤って効かないようにする）
                state.last_stream_fetch.remove(resource_id);
            }

            log::info!(
                "Player initialized for resource: {} ({})",
                resource_id,
                path
            );
        }
        ArchivedResponse::StreamData {
            resource_id,
            data,
            pts,
            dts,
            is_key,
            discontinuous,
            generation,
        } => {
            let pts: Option<i64> = pts.deserialize(&mut rkyv::Infallible).unwrap();
            let dts: Option<i64> = dts.deserialize(&mut rkyv::Infallible).unwrap();

            {
                // デコードは重い処理のため、State ロックを保持したまま行わない
                let stream_players = stream_players_of(state)?;
                if let Some(mut player) = stream_players.get_mut(resource_id) {
                    // パケットをデコードしてフレームを取得
                    player
                        .process_packet(data, pts, dts, *is_key, *discontinuous, *generation)
                        .map_err(|e| EsotereelError::DecodeError(e.to_string()))?;
                }
            }
        }
        ArchivedResponse::StreamDataEnd {
            resource_id,
            fetched_ranges,
            generation: _,
        } => {
            let fetched_ranges: Vec<Range<f64>> =
                fetched_ranges.deserialize(&mut rkyv::Infallible).unwrap();

            let stream_players = stream_players_of(state)?;
            if let Some(mut player) = stream_players.get_mut(resource_id) {
                player.fetch_state = FetchState::Idle;
                player.free_no_needed_frames(&fetched_ranges); // Vec対応版
            }
        }
        ArchivedResponse::DebugProjectStruct(server_str) => {
            let (project_arc, _) = project_and_callback(state)?;
            let project_guard = project_arc.read_or_err()?;

            if let Some(project) = project_guard.as_ref() {
                let client_str = format!("{:#?}", project);

                info!("Client: {}", client_str.green());
            } else {
                info!("Client project is None");
            }

            if let Some(server_str) = server_str.as_deref() {
                info!("Server: {}", server_str.red());
            } else {
                info!("Server project is None");
            }
        }
    }
    Ok(())
}

/// クリッププロパティから動画ソース（`std:video:media/source`）のパス文字列を取り出す。
fn video_source_path(clip: &Clip) -> Option<String> {
    let media_key = NamespacedID::new("std", "video:media").unwrap();
    let FieldValue::Map(media_map) = clip.properties.get(&media_key)?.in_frame_of()? else {
        return None;
    };
    let FieldValue::String(source) = media_map.get("source")? else {
        return None;
    };
    if source.is_empty() {
        return None;
    }
    Some(source.clone())
}

/// プロパティ変更（ルートキー + 値）から動画ソースパスを取り出す。
/// `key` が動画メディアグループのルートで、`value` がそのマップ値である場合にのみ
/// `source` メンバーを返す。
fn video_source_from_property(key: &NamespacedID, value: &ClipBindingValue) -> Option<String> {
    let media_key = NamespacedID::new("std", "video:media").unwrap();
    if key != &media_key {
        return None;
    }
    let FieldValue::Map(media_map) = value.in_frame_of()? else {
        return None;
    };
    let FieldValue::String(source) = media_map.get("source")? else {
        return None;
    };
    if source.is_empty() {
        return None;
    }
    Some(source.clone())
}
