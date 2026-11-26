use std::{
    ops::Range,
    path::{Path, PathBuf},
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
        asset::AssetEntry,
        clip::{Clip, ClipBindingValue},
        ids::{AssetRef, ClipId, LayerFolderId, LayerId, TimelineId},
        layer::LayerMeta,
        layer_outline::{Meta, OutlineNode},
        timeline::TimelineMeta,
        Project,
    },
    responces::{ArchivedResponse, AssetAvailabilityWire, AssetEntryWire},
    util::result::{EsotereelError, EsotereelResult, LockExt},
    StreamState,
};
use log::info;

use crate::state::ClientState;

/// `check_archived_root` で検証済みのペイロードをデシリアライズする。
///
/// エラー型が `Infallible` なので失敗し得ないが、検証の前提が崩れた場合に
/// panic ではなくコンパイルエラーで気付けるよう、網羅的に受ける。
fn deserialize_validated<T, A>(archived: &A) -> T
where
    A: rkyv::Deserialize<T, rkyv::Infallible>,
{
    match archived.deserialize(&mut rkyv::Infallible) {
        Ok(value) => value,
        Err(error) => match error {},
    }
}

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

                    let outline = deserialize_validated(&archived.outline);

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
                let clip: Clip = deserialize_validated(archived_clip);
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
                // ソースがアセット参照（UUID）の場合は登録時のエントリが既に
                // あるはずで、参照文字列をパスとして登録し直してはいけない。
                for source in sources {
                    if !is_asset_ref(&source) {
                        let _ = project.register_asset(Path::new(&source));
                    }
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

                let clip_id: ClipId = deserialize_validated(clip_id);
                let key: PropertyKey = deserialize_validated(key);
                let value: ClipBindingValue = deserialize_validated(value);

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
                if !is_asset_ref(&source) {
                    let _ = project.register_asset(Path::new(&source));
                }
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
            // 次の操作でステールネスエラーが再発しないようにする。
            // 配置（レイヤー索引・チャンク所有）ごと更新するため、
            // 実体だけを書き換えず set_clip_placement を経由する。
            if !timeline.set_clip_placement(*clip_id, *current_position, *current_duration) {
                log::warn!(
                    "CommandRejected: clip {} not found in local timeline",
                    *clip_id
                );
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
                    let id: LayerId = deserialize_validated(archived_id);
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
                    let meta: Meta = deserialize_validated(meta);
                    timeline.apply_outline_folder_meta(*id, meta);
                }
                for (parent, kids) in children.iter() {
                    let parent: Option<LayerFolderId> =
                        deserialize_validated(parent);
                    let kids: Vec<OutlineNode> = deserialize_validated(kids);
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
                    let id: LayerFolderId = deserialize_validated(archived_id);
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
            let pts: Option<i64> = deserialize_validated(pts);
            let dts: Option<i64> = deserialize_validated(dts);

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
                deserialize_validated(fetched_ranges);

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
            let error: EsotereelError = deserialize_validated(error);
            // ログだけでなく GUI が回収する非同期エラー置き場にも残し、
            // ステータスバーで失敗が見えるようにする。
            if let Ok(mut slot) = state.async_error.lock() {
                *slot = Some(error.to_string());
            }
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
        ArchivedResponse::ProjectInspected {
            path,
            version,
            status,
            project_id,
        } => {
            let inspection = crate::state::ProjectInspection {
                path: path.to_string(),
                version_major: version.major,
                version_minor: version.minor,
                version_revision: version.revision,
                status: deserialize_validated(status),
                project_id: project_id.to_string(),
            };
            info!(
                "project inspected: {} (version {}.{}.{}, status {:?})",
                inspection.path,
                inspection.version_major,
                inspection.version_minor,
                inspection.version_revision,
                inspection.status
            );
            if let Ok(mut slot) = state.project_inspection.lock() {
                *slot = Some(inspection);
            }
        }
        ArchivedResponse::ProjectOpened {
            path,
            version,
            status,
        } => {
            let opened = crate::state::ProjectOpenedInfo {
                path: path.to_string(),
                version_major: version.major,
                version_minor: version.minor,
                version_revision: version.revision,
                status: deserialize_validated(status),
            };
            info!("project opened: {}", opened.path);
            if let Ok(mut folder) = state.current_project_folder.write_or_err() {
                *folder = Some(opened.path.clone());
            }
            if let Ok(mut slot) = state.project_opened.lock() {
                *slot = Some(opened);
            }
        }
        ArchivedResponse::ProjectSaved { path } => {
            info!("project saved: {path}");
            if let Ok(mut folder) = state.current_project_folder.write_or_err() {
                *folder = Some(path.to_string());
            }
            if let Ok(mut slot) = state.project_saved.lock() {
                *slot = Some(path.to_string());
            }
        }
        ArchivedResponse::KindTableSync { entries } => {
            // 権威（core）からの全量配信でミラーの表を置き換える。
            let (project_arc, _) = project_and_callback(state);
            let project_guard = project_arc.read_or_err()?;
            if let Some(project) = project_guard.as_ref() {
                let parsed = parse_kind_entries(entries.iter().map(|name| name.as_str()));
                project.replace_kind_table(parsed);
            }
        }
        ArchivedResponse::KindTableAppended { start, entries } => {
            let (project_arc, _) = project_and_callback(state);
            let project_guard = project_arc.read_or_err()?;
            if let Some(project) = project_guard.as_ref() {
                let parsed = parse_kind_entries(entries.iter().map(|name| name.as_str()));
                if !project.append_kind_entries(*start, &parsed) {
                    log::error!(
                        "kind table append from server disagrees with the local mirror (start {start}); keeping local table"
                    );
                }
            }
        }
        ArchivedResponse::AutosaveGenerations { generations } => {
            let generations: Vec<esotereel_lib::responces::AutosaveGenerationWire> =
                deserialize_validated(generations);
            info!("autosave generations received: {}", generations.len());
            if let Ok(mut slot) = state.autosave_generations.lock() {
                *slot = generations;
            }
        }
        ArchivedResponse::AutosaveRestored { generation_id } => {
            info!("autosave generation restored: {generation_id}");
        }
        ArchivedResponse::AssetTableSync { entries } => {
            let entries: Vec<AssetEntryWire> = deserialize_validated(entries);
            info!("asset table sync received: {} entries", entries.len());
            // 同期済みアセット表を差し替える（一覧表示用の FFI が読む）。
            if let Ok(mut slot) = state.synced_assets.lock() {
                *slot = entries.clone();
            }
            // ミラーのレジストリを権威側の内容へ収束させる。全量配信は
            // プロジェクト開始時に限られ、その時点のミラーは ProjectMeta
            // で作り直された直後なので、丸ごと置き換えてよい（ローカルで
            // 登録した分もサーバー側の登録として増分で再配信される）。
            let (project_arc, _) = project_and_callback(state);
            let mut project_guard = project_arc.write_or_err()?;
            if let Some(project) = project_guard.as_mut() {
                let mirrored: Vec<AssetEntry> =
                    entries.iter().filter_map(asset_entry_from_wire).collect();
                project.assets_mut().replace_all(mirrored);
            }
        }
        ArchivedResponse::AssetTableUpdated { upserts, removed } => {
            let upserts: Vec<AssetEntryWire> = deserialize_validated(upserts);
            let removed: Vec<String> = deserialize_validated(removed);
            if let Ok(mut slot) = state.synced_assets.lock() {
                for wire in &upserts {
                    match slot.iter_mut().find(|existing| existing.id == wire.id) {
                        Some(existing) => *existing = wire.clone(),
                        None => slot.push(wire.clone()),
                    }
                }
                if !removed.is_empty() {
                    slot.retain(|wire| !removed.contains(&wire.id));
                }
            }
            let (project_arc, _) = project_and_callback(state);
            let mut project_guard = project_arc.write_or_err()?;
            if let Some(project) = project_guard.as_mut() {
                for wire in &upserts {
                    if let Some(entry) = asset_entry_from_wire(wire) {
                        project.assets_mut().upsert_entry(entry);
                    }
                }
                for id in &removed {
                    if let Ok(asset_id) = id.parse::<AssetRef>() {
                        let _ = project.assets_mut().remove(asset_id);
                    }
                }
            }
        }
    }
    Ok(())
}

/// 配信されたアセット転送形をミラー側のレジストリエントリへ戻します。
///
/// fingerprint の hash 本体は配信に含まれないため、ミラー側では未計算
/// として扱います（パスの解決に hash は不要で、後埋めはサーバー側の
/// 役目です）。解釈できない ID は警告を残して除きます。
fn asset_entry_from_wire(wire: &AssetEntryWire) -> Option<AssetEntry> {
    use esotereel_lib::project::asset::{
        AssetAvailability, AssetFingerprint, AssetLocation,
    };
    let id: AssetRef = match wire.id.parse() {
        Ok(id) => id,
        Err(error) => {
            log::warn!(
                "ignoring asset with unparseable id `{}` from server: {error}",
                wire.id
            );
            return None;
        }
    };
    let source = match &wire.availability {
        AssetAvailabilityWire::External { path } => AssetAvailability::Available(
            AssetLocation::External {
                path: PathBuf::from(path),
            },
        ),
        AssetAvailabilityWire::Managed { path } => AssetAvailability::Available(
            AssetLocation::Managed {
                path: PathBuf::from(path),
            },
        ),
        AssetAvailabilityWire::Missing { previous_path } => AssetAvailability::Missing {
            previous_path: PathBuf::from(previous_path),
        },
    };
    Some(AssetEntry {
        id,
        name: wire.name.clone(),
        source,
        fingerprint: wire
            .fingerprint_size
            .map(|size| AssetFingerprint { size, hash: None }),
    })
}

/// 配信された kind 名一覧を解釈します。解釈できない名前は警告を残して
/// 除きます（連番がずれるため、以降の追加配信は整合チェックで弾かれます）。
fn parse_kind_entries<'a>(entries: impl Iterator<Item = &'a str>) -> Vec<NamespacedID> {
    entries
        .filter_map(|name| match NamespacedID::parse(name) {
            Ok(kind) => Some(kind),
            Err(error) => {
                log::warn!("ignoring unparseable kind name `{name}` from server: {error}");
                None
            }
        })
        .collect()
}

/// ソース文字列がアセット参照（UUID 形式）かどうか。
fn is_asset_ref(source: &str) -> bool {
    uuid::Uuid::parse_str(source).is_ok()
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
