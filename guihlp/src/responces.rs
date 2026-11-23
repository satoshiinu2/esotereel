use std::{
    ops::Range,
    path::Path,
    sync::{Arc, RwLock},
};

use colored::Colorize;
use esotereel_lib::{
    decode::streamplayer::{FetchState, StreamPlayer},
    plugin::{
        clip::component::{ComponentCapability, RuntimeDefinition},
        property::{value::FieldValue, PropertyKey},
        NamespacedID, PluginLoader,
    },
    project::{
        clip::{Clip, ClipBindingValue},
        ids::{ClipId, LayerFolderId, LayerId, TimelineId},
        layer::LayerMeta,
        layer_outline::{Meta, OutlineNode},
        timeline::TimelineMeta,
        Project,
    },
    responces::ArchivedResponse,
    util::result::{EsotereelError, EsotereelResult, LockExt},
    StreamState,
};
use log::info;
use rkyv::Deserialize;

use crate::state::ClientState;

/// project の `Arc` とコールバック関数ポインタを取り出す。
/// ClientState 自体にロックは無く、メンバーはフィールド側の同期で保護される。
fn project_and_callback(
    state: &ClientState,
) -> (Arc<RwLock<Option<Project>>>, extern "C" fn(TimelineId)) {
    (state.project_arc(), state.gui_callbacks.mark_dirty_timeline)
}

/// `plugin_loader` の `Arc` だけを取り出す。
fn plugin_loader_of(state: &ClientState) -> Arc<RwLock<PluginLoader>> {
    state.plugin_loader_arc()
}

/// デコード等の重い処理を他のロックなしで行うため、stream_players の
/// `Arc` だけを取り出す。
fn stream_players_of(
    state: &ClientState,
) -> Arc<dashmap::DashMap<esotereel_lib::project::ids::ResourceId, StreamPlayer>> {
    state.stream_players_arc()
}

pub(super) fn on_responce_recveve(
    responce: &ArchivedResponse,
    state: &ClientState,
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

            // 差し替えは Arc の付け替えではなく、既存の RwLock への
            // インプレース書き込みで行う。C++ が read ガードを保持中なら
            // その解放まで待つ（ガードはフレーム単位で短命）。
            *state.project.write_or_err()? = Some(real_project);
            // プロジェクトが再初期化されたため、タイムラインごとの最後の
            // FetchClipsInRange 送信記録をクリアする（新しいプロジェクトでの
            // 重複送信抑制が誤って働かないようにする）
            state.last_clips_fetch.clear();

            let fn_ptr = state.gui_callbacks.mark_dirty_timeline;

            for i in 0..timeline_len {
                fn_ptr(i as TimelineId);
            }
        }
        ArchivedResponse::UpdateClip { timeline_id, clips } => {
            let (project_arc, fn_ptr) = project_and_callback(state);
            let loader = plugin_loader_of(state);

            // Project ロックを取る前にデシリアライズと動画ソースの解決を済ませる。
            // ソースパスはクリップ種別の能力宣言から解決し、プロパティ ID を
            // ハードコードしない（AGENTS.md 第9条）。
            let mut deserialized: Vec<(LayerId, Clip)> = Vec::new();
            let mut sources = Vec::new();
            let loader_guard = loader.read_or_err().ok();
            for (layer_id, archived_clip) in clips.iter() {
                let clip: Clip = archived_clip.deserialize(&mut rkyv::Infallible).unwrap();
                if let Some(loader_guard) = loader_guard.as_ref() {
                    if let Ok(definition) = loader_guard.resolve_component_definition(&clip.kind_id)
                    {
                        if let Some(source) = video_source_path(&definition, &clip) {
                            sources.push(source);
                        }
                    }
                }
                deserialized.push((*layer_id, clip));
            }

            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                for (layer_id, clip) in deserialized {
                    timeline.upsert_clip_from_network(layer_id, clip);
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
            let (project_arc, fn_ptr) = project_and_callback(state);
            let loader = plugin_loader_of(state);
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
                let key: PropertyKey = key.deserialize(&mut rkyv::Infallible).unwrap();
                let value: ClipBindingValue = value.deserialize(&mut rkyv::Infallible).unwrap();

                // プロパティ変更が動画ソースなら、タイムライン貸し出しを解放した後に
                // クライアントのアセットレジストリにも反映する。対象かどうかは
                // クリップ種別の能力宣言から解決する（AGENTS.md 第9条）。
                // ロック順序は Project → PluginLoader（docs/architecture/lock-order.md）。
                if let Some(clip) = timeline.get_clip(clip_id) {
                    if let Ok(loader_guard) = loader.read_or_err() {
                        if let Ok(definition) =
                            loader_guard.resolve_component_definition(&clip.kind_id)
                        {
                            new_source = video_source_from_property(&definition, &key, &value);
                        }
                    }
                }

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
            let (project_arc, fn_ptr) = project_and_callback(state);
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
            let (project_arc, fn_ptr) = project_and_callback(state);
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
            let (project_arc, fn_ptr) = project_and_callback(state);
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
            let (project_arc, fn_ptr) = project_and_callback(state);
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
            let (project_arc, fn_ptr) = project_and_callback(state);
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
            let (project_arc, fn_ptr) = project_and_callback(state);
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
                // DashMap への挿入のみ（各フィールドの内部同期で保護される）
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
                let stream_players = stream_players_of(state);
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

            let stream_players = stream_players_of(state);
            if let Some(mut player) = stream_players.get_mut(resource_id) {
                player.fetch_state = FetchState::Idle;
                player.free_no_needed_frames(&fetched_ranges); // Vec対応版
            }
        }
        ArchivedResponse::AssetImported { asset_id } => {
            info!("asset import succeeded: {asset_id}");
        }
        ArchivedResponse::Failed { error } => {
            let error: EsotereelError = error.deserialize(&mut rkyv::Infallible).unwrap();
            return Err(error.into());
        }
        ArchivedResponse::DebugProjectStruct(server_str) => {
            let (project_arc, _) = project_and_callback(state);
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

/// クリッププロパティから動画ソースのパス文字列を取り出す。
///
/// パスはクリップ種別の能力宣言（`FrameProvider` プロパティの `media-source`
/// メンバー）から解決し、プロパティ ID をハードコードしない（AGENTS.md 第9条）。
fn video_source_path(definition: &RuntimeDefinition, clip: &Clip) -> Option<String> {
    let path = definition.member_path(
        &ComponentCapability::FrameProvider,
        &ComponentCapability::MediaSource,
    )?;
    let FieldValue::String(source) = clip.get_property_path(&path)? else {
        return None;
    };
    if source.is_empty() {
        return None;
    }
    Some(source.clone())
}

/// プロパティ変更（ルートキー + 値）から動画ソースパスを取り出す。
/// `key` が `FrameProvider` プロパティのルートで、`value` がそのマップ値である
/// 場合にのみ `media-source` メンバーを返す。ルートとメンバーはクリップ種別の
/// 能力宣言から解決する（AGENTS.md 第9条）。
fn video_source_from_property(
    definition: &RuntimeDefinition,
    key: &PropertyKey,
    value: &ClipBindingValue,
) -> Option<String> {
    let property = definition.property_by_capability(&ComponentCapability::FrameProvider)?;
    if key != &property.key {
        return None;
    }
    let member = definition.member_key_by_capability(&ComponentCapability::MediaSource)?;
    let FieldValue::Map(media_map) = value.in_frame_of()? else {
        return None;
    };
    let FieldValue::String(source) = media_map.get(member)? else {
        return None;
    };
    if source.is_empty() {
        return None;
    }
    Some(source.clone())
}

#[cfg(test)]
#[path = "../tests/unit/responces_lock_order_stress_tests.rs"]
mod lock_order_stress_tests;
