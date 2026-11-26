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
    path::{Path, PathBuf},
    sync::{
        atomic::Ordering,
        Arc,
    },
};

use esotereel_lib::{
    project::{
        asset::hash_file_blake3,
        autosave::{list_autosave_generations, restore_autosave_generation},
        ids::AssetRef,
        save::{backup_project_folder, inspect_project, load_project, save_project},
        Project,
    },
    responces::{AutosaveGenerationWire, Response},
    settings::AppSettings,
    util::result::{EsotereelError, LockExt, MutexExt},
};

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
        project.attach_folder_chunk_stores(&dir);
        anyhow::Ok((project, dir.clone(), version, status))
    })();
    match result {
        Ok((project, dir, version, status)) => {
            if let Err(error) = install_project(state, client_id, project, Some(dir.clone())) {
                send_failed(state, client_id, "open project", &error);
                return;
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
