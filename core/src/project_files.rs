//! プロジェクトフォルダの開閉・保存、オートセーブ復旧、kind 表の配信、
//! fingerprint の後埋めなど、プロジェクト単位のリクエスト処理です。
//!
//! フォルダモデル（設計決定）:
//! - 新規プロジェクトは `data_dir/temp/<プロジェクト ID>` の仮フォルダで
//!   開始し、名前付き保存で実フォルダへ移ります。
//! - 空でないフォルダへの保存は、リクエストの `overwrite` フラグが
//!   無ければ失敗します（黙って上書きしない）。
//! - 開くときはバージョン判定をクライアントへ返し、段階表示は GUI 側が
//!   行います。バックアップ指定時は開く前にフォルダを複製します。

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::Ordering,
        Arc,
    },
};

use esotereel_lib::{
    plugin::{
        property::{PropertyPath, value::FieldValue},
        NamespacedID, PluginLoader,
    },
    project::{
        asset::hash_file_blake3,
        autosave::{list_autosave_generations, restore_autosave_generation},
        clip::get_property_path_from_map,
        ids::{AssetRef, ClipId, TimelineId},
        save::{backup_project_folder, inspect_project, load_project, save_project},
        Project,
    },
    responces::{AssetEntryWire, AutosaveGenerationWire, Response},
    settings::AppSettings,
    util::result::{EsotereelError, LockExt, MutexExt},
};

use crate::project::MediaPropertyPaths;
use crate::state::ServerState;

/// 失敗をクライアントへ `Response::Failed` で返します。
pub fn send_failed(
    state: &ServerState,
    client_id: u32,
    context: &str,
    error: &anyhow::Error,
) {
    log::error!("{context} failed: {error:#}");
    state.network.send(
        client_id,
        &Response::Failed {
            error: EsotereelError::CommandFailed(format!("{context}: {error:#}")),
        },
    );
}

/// プロジェクトを差し替え、開始バンドル（`ProjectMeta` ＋ kind 表の
/// 全量配信）をクライアントへ送ります。
///
/// ロック保持中に送信しないよう、差し替えと履歴クリアを済ませてから
/// 送信します（lock-order.md）。
pub fn install_project(
    state: &ServerState,
    client_id: u32,
    project: Project,
    folder: Option<PathBuf>,
) -> anyhow::Result<()> {
    let timelines = project.timelines_meta();
    let kind_entries: Vec<String> = project
        .kind_table()
        .entries()
        .iter()
        .map(|kind| kind.full().to_string())
        .collect();
    {
        let mut project_guard = state.project.write_or_err()?;
        state.histories.lock_or_err()?.clear();
        *project_guard = Some(project);
        let mut folder_guard = state.project_folder.write_or_err()?;
        *folder_guard = folder;
    }
    *state.kind_sync_len.lock_or_err()? = kind_entries.len();

    for timeline in &timelines {
        state
            .network
            .update_client_view(client_id, timeline.id, i64::MIN..i64::MAX);
    }
    state
        .network
        .send(client_id, &Response::ProjectMeta { timelines });
    state
        .network
        .send(client_id, &Response::KindTableSync { entries: kind_entries });
    send_asset_table_sync(state, client_id)?;
    Ok(())
}

/// 現在の kind 表の全量をクライアントへ送り、配信済み長さを記録します。
pub fn send_kind_table_sync(state: &ServerState, client_id: u32) -> anyhow::Result<()> {
    let entries: Vec<String> = {
        let project_guard = state.project.read_or_err()?;
        let Some(project) = project_guard.as_ref() else {
            return Ok(());
        };
        project
            .kind_table()
            .entries()
            .iter()
            .map(|kind| kind.full().to_string())
            .collect()
    };
    *state.kind_sync_len.lock_or_err()? = entries.len();
    state
        .network
        .send(client_id, &Response::KindTableSync { entries });
    Ok(())
}

/// kind 表が伸びていたら、増分だけを全クライアントへ配信します。
///
/// 変更イベントのたびに呼ばれます。プロジェクトが差し替わって表が
/// 縮んでいたら、配信済み長さを戻すだけにします（全量配信は開始
/// バンドル側の役目）。
pub fn broadcast_kind_appends(state: &ServerState) {
    let entries: Vec<String> = {
        let Ok(project_guard) = state.project.read_or_err() else {
            return;
        };
        let Some(project) = project_guard.as_ref() else {
            return;
        };
        project
            .kind_table()
            .entries()
            .iter()
            .map(|kind| kind.full().to_string())
            .collect()
    };
    let Ok(mut synced) = state.kind_sync_len.lock_or_err() else {
        return;
    };
    if entries.len() > *synced {
        let start = *synced as u32;
        let appended = entries[*synced..].to_vec();
        *synced = entries.len();
        drop(synced);
        state.network.send_all(&Response::KindTableAppended {
            start,
            entries: appended,
        });
    } else if entries.len() < *synced {
        *synced = entries.len();
    }
}

/// 全タイムラインの全クリップがメディア source として参照している
/// アセットの集合を返します。
///
/// source はクリップ種別の能力宣言（`FrameProvider` の `media-source`
/// メンバー）から解決し、プロパティ ID をハードコードしません
/// （AGENTS.md 第9条）。`AssetRef` 型の値だけを参照として数え、未設定
/// （nil UUID）は対象外です。旧形式の文字列 source は、プロジェクトを
/// 開くときの変換で型付き値になるため、ここでは扱いません。実体が
/// アンロードされているチャンクのクリップは走査に含まれないため、
/// 正確な判定が要る呼び出し側は事前に `ensure_all_chunks_loaded` で
/// 読み込んでおくこと。
pub fn used_asset_refs(project: &Project, loader: &PluginLoader) -> HashSet<AssetRef> {
    let mut used = HashSet::new();
    let mut source_paths: HashMap<NamespacedID, Option<PropertyPath>> = HashMap::new();
    for (_, timeline) in project.iter_timelines() {
        for (_, clip) in timeline.iter_clips() {
            let source_path = source_paths
                .entry(clip.kind_id.clone())
                .or_insert_with(|| {
                    loader
                        .resolve_component_definition(&clip.kind_id)
                        .ok()
                        .and_then(|definition| MediaPropertyPaths::resolve(&definition).source)
                });
            let Some(source_path) = source_path else {
                continue;
            };
            if let Some(FieldValue::AssetRef(asset_ref)) =
                get_property_path_from_map(&clip.properties, source_path)
            {
                if !asset_ref.is_nil() {
                    used.insert(*asset_ref);
                }
            }
        }
    }
    used
}

/// 旧形式（文字列）のメディア source を `AssetRef` 値へ変換します。
///
/// 形式 1.0.0 では source が文字列（UUID 文字列または生パス）で保存されて
/// いました。変換にはプラグイン定義からのパス解決が要るため、lib の
/// 読み込みではなく、プロジェクトを開くここで行います。変換は冪等で、
/// 変換した件数を返します。個々の変換はログに残します。
pub fn convert_legacy_media_sources(project: &mut Project, loader: &PluginLoader) -> usize {
    let mut source_paths: HashMap<NamespacedID, Option<PropertyPath>> = HashMap::new();
    let mut targets: Vec<(TimelineId, ClipId, PropertyPath)> = Vec::new();
    for (timeline_id, timeline) in project.iter_timelines() {
        for (clip_id, clip) in timeline.iter_clips() {
            let source_path = source_paths
                .entry(clip.kind_id.clone())
                .or_insert_with(|| {
                    loader
                        .resolve_component_definition(&clip.kind_id)
                        .ok()
                        .and_then(|definition| MediaPropertyPaths::resolve(&definition).source)
                });
            let Some(source_path) = source_path else {
                continue;
            };
            if matches!(
                get_property_path_from_map(&clip.properties, source_path),
                Some(FieldValue::String(_))
            ) {
                targets.push((*timeline_id, *clip_id, source_path.clone()));
            }
        }
    }

    let mut converted = 0;
    for (timeline_id, clip_id, source_path) in targets {
        // 変換ではレジストリへの登録で project を借用するため、タイムラインの
        // 借用と重ねないようプロパティを取り出してから書き戻す。
        let mut properties = {
            let Some(timeline) = project.timeline_mut(timeline_id) else {
                continue;
            };
            let Some(clip) = timeline.get_clip_mut(clip_id) else {
                continue;
            };
            std::mem::take(&mut clip.properties)
        };
        if crate::project::resolve_source_to_asset_ref(
            project.assets_mut(),
            &mut properties,
            &source_path,
        ) {
            converted += 1;
            log::info!(
                "converted legacy string media source of clip {clip_id} to an asset reference"
            );
        }
        let Some(timeline) = project.timeline_mut(timeline_id) else {
            continue;
        };
        if let Some(clip) = timeline.get_clip_mut(clip_id) {
            clip.properties = properties;
        }
    }
    converted
}

/// 現在のアセット表を転送形で組み立てます。
///
/// `in_use` は読み込み済みチャンクの走査で判定し、配信済みスナップ
/// ショットで使用中だったものは維持します（未読み込みチャンクの
/// クリップを取りこぼして「未使用」に倒れないようにするため。
/// 正確な判定は全量配信と削除時の再確認で行います）。
fn current_asset_wires(state: &ServerState) -> Vec<AssetEntryWire> {
    let entries = {
        let Ok(project_guard) = state.project.read_or_err() else {
            return Vec::new();
        };
        let Some(project) = project_guard.as_ref() else {
            return Vec::new();
        };
        let Ok(loader) = state.common.plugin_loader.read_or_err() else {
            return Vec::new();
        };
        let used = used_asset_refs(project, &loader);
        project
            .assets()
            .iter()
            .map(|entry| AssetEntryWire::from_entry(entry, used.contains(&entry.id)))
            .collect::<Vec<_>>()
    };
    let Ok(snapshot) = state.asset_sync_snapshot.lock_or_err() else {
        return entries;
    };
    entries
        .into_iter()
        .map(|mut wire| {
            if !wire.in_use {
                if let Some(previous) = snapshot.get(&wire.id) {
                    wire.in_use = previous.in_use;
                }
            }
            wire
        })
        .collect()
}

/// アセット表の全量をクライアントへ送り、配信済みスナップショットを
/// 作り直します（プロジェクト開始バンドル用）。
///
/// `in_use` は全チャンクを読み込んでから判定する正確な値です。
pub fn send_asset_table_sync(state: &ServerState, client_id: u32) -> anyhow::Result<()> {
    let entries = {
        let mut project_guard = state.project.write_or_err()?;
        let project = project_guard
            .as_mut()
            .ok_or(EsotereelError::ProjectNotFound)?;
        project.ensure_all_chunks_loaded()?;
        let loader = state.common.plugin_loader.read_or_err()?;
        let used = used_asset_refs(project, &loader);
        project
            .assets()
            .iter()
            .map(|entry| AssetEntryWire::from_entry(entry, used.contains(&entry.id)))
            .collect::<Vec<_>>()
    };
    *state.asset_sync_snapshot.lock_or_err()? = entries
        .iter()
        .map(|wire| (wire.id.clone(), wire.clone()))
        .collect();
    state
        .network
        .send(client_id, &Response::AssetTableSync { entries });
    Ok(())
}

/// アセット表が配信済みスナップショットから変わっていたら、差分だけを
/// 全クライアントへ配信します。
///
/// 変更イベントのたびと、アセットを直接触るリクエストの成功後に
/// 呼ばれます。差分の検出は転送形の比較なので、登録・名前変更・
/// fingerprint の後埋めなど、どの経路の変更でも拾えます。
pub fn broadcast_asset_updates(state: &ServerState) {
    let entries = current_asset_wires(state);
    let Ok(mut snapshot) = state.asset_sync_snapshot.lock_or_err() else {
        return;
    };
    let mut upserts: Vec<AssetEntryWire> = Vec::new();
    let mut current_ids: HashSet<&str> = HashSet::new();
    for wire in &entries {
        current_ids.insert(wire.id.as_str());
        match snapshot.get(&wire.id) {
            Some(previous) if previous == wire => {}
            _ => upserts.push(wire.clone()),
        }
    }
    let removed: Vec<String> = snapshot
        .keys()
        .filter(|id| !current_ids.contains(id.as_str()))
        .cloned()
        .collect();
    *snapshot = entries
        .into_iter()
        .map(|wire| (wire.id.clone(), wire))
        .collect();
    drop(snapshot);
    if !upserts.is_empty() || !removed.is_empty() {
        state.network.send_all(&Response::AssetTableUpdated { upserts, removed });
    }
}

/// アセットの表示名を変更します。
pub fn handle_rename_asset(state: &ServerState, client_id: u32, asset_id: &str, name: &str) {
    let result = (|| -> anyhow::Result<()> {
        let id = parse_asset_id(asset_id)?;
        let mut project_guard = state.project.write_or_err()?;
        let project = project_guard
            .as_mut()
            .ok_or(EsotereelError::ProjectNotFound)?;
        project.assets_mut().rename(id, name.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            state.project_dirty.store(true, Ordering::SeqCst);
            broadcast_asset_updates(state);
        }
        Err(error) => send_failed(state, client_id, "rename asset", &error),
    }
}

/// アセットをレジストリから削除します。
///
/// クリップが参照しているアセットは黙って消さず、失敗として返します
/// （参照の判定は全チャンクを読み込んでから正確に行います）。
pub fn handle_remove_asset(state: &ServerState, client_id: u32, asset_id: &str) {
    let result = (|| -> anyhow::Result<()> {
        let id = parse_asset_id(asset_id)?;
        let mut project_guard = state.project.write_or_err()?;
        let project = project_guard
            .as_mut()
            .ok_or(EsotereelError::ProjectNotFound)?;
        project.ensure_all_chunks_loaded()?;
        let loader = state.common.plugin_loader.read_or_err()?;
        if used_asset_refs(project, &loader).contains(&id) {
            anyhow::bail!("asset {id} is still referenced by one or more clips");
        }
        drop(loader);
        project.assets_mut().remove(id)?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            state.project_dirty.store(true, Ordering::SeqCst);
            broadcast_asset_updates(state);
        }
        Err(error) => send_failed(state, client_id, "remove asset", &error),
    }
}

/// アセットの参照先を別の外部ファイルへ付け替えます。
///
/// fingerprint はサイズを取り直して hash は未計算に戻り、登録時と
/// 同じ後埋め（`spawn_fingerprint_backfill`）で再計算されます。
pub fn handle_relink_asset(state: &Arc<ServerState>, client_id: u32, asset_id: &str, path: &str) {
    let result = (|| -> anyhow::Result<()> {
        let id = parse_asset_id(asset_id)?;
        let new_path = Path::new(path);
        if !new_path.is_file() {
            anyhow::bail!("asset source `{path}` is not an existing file");
        }
        let mut project_guard = state.project.write_or_err()?;
        let project = project_guard
            .as_mut()
            .ok_or(EsotereelError::ProjectNotFound)?;
        project.assets_mut().relink_external(id, new_path)?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            state.project_dirty.store(true, Ordering::SeqCst);
            broadcast_asset_updates(state);
            spawn_fingerprint_backfill(state);
        }
        Err(error) => send_failed(state, client_id, "relink asset", &error),
    }
}

fn parse_asset_id(asset_id: &str) -> anyhow::Result<AssetRef> {
    asset_id
        .parse::<AssetRef>()
        .map_err(|error| anyhow::anyhow!("invalid asset id `{asset_id}`: {error}"))
}

/// 新規・復元プロジェクト用の仮フォルダ（`data_dir/temp/<ID>`）を作ります。
pub fn temp_project_folder(state: &ServerState, project_id: uuid::Uuid) -> anyhow::Result<PathBuf> {
    let dir = state
        .dir
        .data_dir()?
        .join("temp")
        .join(project_id.to_string());
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 仮フォルダかどうか（`data_dir/temp/` 配下か）を返します。
fn is_temp_folder(state: &ServerState, folder: &Path) -> bool {
    let Ok(temp_root) = state.dir.data_dir().map(|dir| dir.join("temp")) else {
        return false;
    };
    folder.starts_with(&temp_root)
}

/// 現在の階層設定を読みます（ユーザー設定＋プロジェクト設定）。
///
/// 読み込みに失敗したら警告を残して既定値で続けます（設定ファイルの
/// 不備でサーバー自体を止めないため。未知キーの無視と同じく、設定は
/// ベストエフォートで読む層です）。
pub fn load_app_settings(state: &ServerState) -> AppSettings {
    let user_path = state.dir.client_settings_path().ok();
    let project_dir = state
        .project_folder
        .read_or_err()
        .ok()
        .and_then(|guard| guard.clone());
    match AppSettings::load(user_path.as_deref(), project_dir.as_deref(), None) {
        Ok(settings) => settings,
        Err(error) => {
            log::warn!("failed to load app settings, using defaults: {error:#}");
            AppSettings::default()
        }
    }
}

/// プロジェクトフォルダを開かずにバージョンだけ確認します。
pub fn handle_inspect_project(state: &ServerState, client_id: u32, path: &str) {
    let dir = PathBuf::from(path);
    match inspect_project(&dir) {
        Ok((version, status, project_id)) => {
            state.network.send(
                client_id,
                &Response::ProjectInspected {
                    path: path.to_string(),
                    version: version.into(),
                    status: status.into(),
                    project_id: project_id.to_string(),
                },
            );
        }
        Err(error) => send_failed(state, client_id, "inspect project", &error.into()),
    }
}

/// プロジェクトフォルダを開きます。
pub fn handle_open_project(state: &Arc<ServerState>, client_id: u32, path: &str, backup: bool) {
    let dir = PathBuf::from(path);
    let result = (|| {
        if backup {
            let backup_dir = backup_project_folder(&dir)?;
            log::info!("project backup created at {}", backup_dir.display());
        }
        let loaded = load_project(&dir)?;
        let version = loaded.version.into();
        let status = loaded.status.into();
        let mut project = loaded.project;
        // 旧形式の文字列 source は、開くときに AssetRef 値へ変換する。
        let converted = {
            let loader = state.common.plugin_loader.read_or_err()?;
            convert_legacy_media_sources(&mut project, &loader)
        };
        if converted > 0 {
            log::info!("converted {converted} legacy media source(s) to asset references");
        }
        project.attach_folder_chunk_stores(&dir);
        anyhow::Ok((project, dir.clone(), version, status, converted))
    })();
    match result {
        Ok((project, dir, version, status, converted)) => {
            if let Err(error) = install_project(state, client_id, project, Some(dir.clone())) {
                send_failed(state, client_id, "open project", &error);
                return;
            }
            if converted > 0 {
                // 変換結果をオートセーブで永続化できるよう、変更ありにする。
                state.project_dirty.store(true, Ordering::SeqCst);
            }
            state.network.send(
                client_id,
                &Response::ProjectOpened {
                    path: dir.to_string_lossy().into_owned(),
                    version,
                    status,
                },
            );
            spawn_fingerprint_backfill(state);
        }
        Err(error) => send_failed(state, client_id, "open project", &error),
    }
}

/// 現在のプロジェクトフォルダへ保存します。
pub fn handle_save_project(state: &ServerState, client_id: u32, overwrite: bool) {
    let result = (|| -> anyhow::Result<PathBuf> {
        let folder = state
            .project_folder
            .read_or_err()?
            .clone()
            .ok_or_else(|| anyhow::anyhow!("project has no folder yet; use save-as"))?;
        let mut project_guard = state.project.write_or_err()?;
        let project = project_guard
            .as_mut()
            .ok_or(EsotereelError::ProjectNotFound)?;
        guard_foreign_project(&folder, project.id(), overwrite)?;
        project.ensure_all_chunks_loaded()?;
        save_project(project, &folder)?;
        Ok(folder)
    })();
    finish_save(state, client_id, result);
}

/// 指定フォルダへ名前を付けて保存し、そのフォルダへ移ります。
pub fn handle_save_project_as(
    state: &ServerState,
    client_id: u32,
    path: &str,
    overwrite: bool,
) {
    let target = PathBuf::from(path);
    let result = (|| -> anyhow::Result<PathBuf> {
        let mut project_guard = state.project.write_or_err()?;
        let project = project_guard
            .as_mut()
            .ok_or(EsotereelError::ProjectNotFound)?;
        let current = state.project_folder.read_or_err()?.clone();
        if current.as_deref() == Some(target.as_path()) {
            project.ensure_all_chunks_loaded()?;
            save_project(project, &target)?;
            return Ok(target);
        }
        prepare_save_target(&target, project.id(), overwrite)?;
        project.ensure_all_chunks_loaded()?;
        save_project(project, &target)?;
        project.attach_folder_chunk_stores(&target);
        let old_folder = {
            let mut folder_guard = state.project_folder.write_or_err()?;
            folder_guard.replace(target.clone())
        };
        // 仮フォルダから実フォルダへ移ったら、仮フォルダは片付けます。
        if let Some(old) = old_folder {
            if is_temp_folder(state, &old) && old != target {
                if let Err(error) = std::fs::remove_dir_all(&old) {
                    log::warn!("failed to remove temp project folder {}: {error}", old.display());
                }
            }
        }
        Ok(target)
    })();
    finish_save(state, client_id, result);
}

fn finish_save(state: &ServerState, client_id: u32, result: anyhow::Result<PathBuf>) {
    match result {
        Ok(folder) => {
            state.project_dirty.store(false, Ordering::SeqCst);
            state.network.send(
                client_id,
                &Response::ProjectSaved {
                    path: folder.to_string_lossy().into_owned(),
                },
            );
        }
        Err(error) => send_failed(state, client_id, "save project", &error),
    }
}

/// フォルダに別のプロジェクトの `project.json` がある場合、
/// `overwrite` が無ければ保存を拒否します。
fn guard_foreign_project(folder: &Path, project_id: uuid::Uuid, overwrite: bool) -> anyhow::Result<()> {
    if overwrite || !folder.join("project.json").exists() {
        return Ok(());
    }
    let (_, _, existing_id) = inspect_project(folder)?;
    if existing_id != project_id {
        anyhow::bail!(
            "folder {} contains a different project ({existing_id}); refusing to overwrite without confirmation",
            folder.display()
        );
    }
    Ok(())
}

/// 名前付き保存先を整えます。空でないフォルダは `overwrite` が無ければ
/// 拒否し、別プロジェクトのフォルダを上書きする場合は中身を消してから
/// 保存します（古いプロジェクトの残骸を混ぜないため）。
fn prepare_save_target(target: &Path, project_id: uuid::Uuid, overwrite: bool) -> anyhow::Result<()> {
    if !target.exists() {
        return Ok(());
    }
    let has_entries = std::fs::read_dir(target)?.next().is_some();
    if !has_entries {
        return Ok(());
    }
    if !overwrite {
        anyhow::bail!(
            "folder {} is not empty; refusing to save without overwrite confirmation",
            target.display()
        );
    }
    if target.join("project.json").exists() {
        let (_, _, existing_id) = inspect_project(target)?;
        if existing_id == project_id {
            // 自分のプロジェクトの再保存は、保存側の掃除に任せる。
            return Ok(());
        }
    }
    std::fs::remove_dir_all(target)?;
    std::fs::create_dir_all(target)?;
    Ok(())
}

/// オートセーブ世代の一覧を返します。
pub fn handle_list_autosave_generations(state: &ServerState, client_id: u32) {
    let result = (|| -> anyhow::Result<Vec<AutosaveGenerationWire>> {
        let data_dir = state.dir.data_dir()?;
        let generations = list_autosave_generations(&data_dir, None)?;
        Ok(generations.into_iter().map(Into::into).collect())
    })();
    match result {
        Ok(generations) => {
            state
                .network
                .send(client_id, &Response::AutosaveGenerations { generations });
        }
        Err(error) => send_failed(state, client_id, "list autosave generations", &error),
    }
}

/// オートセーブ世代から復元して開きます。
///
/// 復元先は仮フォルダで、通常の保存と同じ形に書き出してから開きます
/// （保存済み・未保存の区別なく同じ扱いにするため）。
pub fn handle_restore_autosave_generation(
    state: &Arc<ServerState>,
    client_id: u32,
    generation_id: &str,
) {
    let result = (|| -> anyhow::Result<(Project, PathBuf)> {
        let data_dir = state.dir.data_dir()?;
        let loaded = restore_autosave_generation(&data_dir, generation_id)?;
        let mut project = loaded.project;
        // 旧形式の世代には文字列 source が残っていることがあるため、
        // 書き出し直す前に AssetRef 値へ変換する。
        {
            let loader = state.common.plugin_loader.read_or_err()?;
            convert_legacy_media_sources(&mut project, &loader);
        }
        let folder = temp_project_folder(state, project.id())?;
        project.ensure_all_chunks_loaded()?;
        save_project(&project, &folder)?;
        project.attach_folder_chunk_stores(&folder);
        Ok((project, folder))
    })();
    match result {
        Ok((project, folder)) => {
            if let Err(error) = install_project(state, client_id, project, Some(folder)) {
                send_failed(state, client_id, "restore autosave generation", &error);
                return;
            }
            state.project_dirty.store(true, Ordering::SeqCst);
            state.network.send(
                client_id,
                &Response::AutosaveRestored {
                    generation_id: generation_id.to_string(),
                },
            );
            spawn_fingerprint_backfill(state);
        }
        Err(error) => send_failed(state, client_id, "restore autosave generation", &error),
    }
}

/// fingerprint の hash が未計算のアセットを、低並行（最大2件）で
/// バックグラウンド後埋めします。
///
/// 対象はプロジェクトを開いた時点と External 登録のたびに呼びます。
/// 再生・エクスポート中の後回しは、判定できる状態信号が現状の core に
/// 無いため行いません（信号ができたらここで間引きます）。
pub fn spawn_fingerprint_backfill(state: &Arc<ServerState>) {
    let state = Arc::clone(state);
    tokio::spawn(async move {
        let targets: Vec<(AssetRef, PathBuf)> = {
            let Ok(project_guard) = state.project.read_or_err() else {
                return;
            };
            let Some(project) = project_guard.as_ref() else {
                return;
            };
            project
                .assets()
                .iter()
                .filter(|entry| entry.fingerprint.as_ref().is_some_and(|fp| fp.hash.is_none()))
                .filter_map(|entry| {
                    project
                        .assets()
                        .resolve(entry.id)
                        .ok()
                        .map(|path| (entry.id, path.to_path_buf()))
                })
                .collect()
        };
        if targets.is_empty() {
            return;
        }
        log::info!(
            "fingerprint backfill: {} asset(s) missing hashes",
            targets.len()
        );
        let semaphore = Arc::new(tokio::sync::Semaphore::new(2));
        let mut handles = Vec::new();
        for (asset_id, path) in targets {
            let Ok(permit) = semaphore.clone().acquire_owned().await else {
                return;
            };
            let state = Arc::clone(&state);
            handles.push(tokio::spawn(async move {
                let _permit = permit;
                let hashed = tokio::task::spawn_blocking(move || hash_file_blake3(&path)).await;
                let Ok(Ok(hash)) = hashed else {
                    return;
                };
                if let Ok(mut project_guard) = state.project.write_or_err() {
                    if let Some(project) = project_guard.as_mut() {
                        // 既に埋まっていたら上書きしない。
                        let missing = project
                            .assets()
                            .get(asset_id)
                            .and_then(|entry| entry.fingerprint.as_ref())
                            .is_some_and(|fp| fp.hash.is_none());
                        if missing {
                            if let Err(error) =
                                project.assets_mut().set_fingerprint_hash(asset_id, hash)
                            {
                                log::warn!(
                                    "fingerprint backfill failed for {asset_id}: {error:#}"
                                );
                            }
                        }
                    }
                }
            }));
        }
        for handle in handles {
            let _ = handle.await;
        }
    });
}

#[cfg(test)]
#[path = "../tests/unit/project_files_tests.rs"]
mod tests;
