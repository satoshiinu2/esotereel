//! オートセーブです。Microsoft Word 式の復元モデルです。
//!
//! 保存済み・未保存のプロジェクトを同じ扱いで、プロジェクトフォルダとは
//! 別の `data_dir/autosave/` に世代アーカイブとして保持します。
//! プロジェクトフォルダをオートセーブで汚しません。
//!
//! - 1世代はメタ一式（project.json / timeline.json / overview / chunks の
//!   JSON）を1つの zstd 圧縮 tar アーカイブにまとめたものです。
//!   アセット実体は含めず参照のみです。
//! - 世代とプロジェクトの紐付けはパスではなく project.json の
//!   プロジェクト ID で行います。
//! - 保持はプロジェクトごとに最新 N 世代まで＋全体の容量上限を超えたら
//!   古い順に削除します。N と上限は `AppSettings` から取ります。
//!
//! トリガー（変更後デバウンス・定期実行）は core 側の責務で、ここでは
//! 世代の書き込み・一覧・復旧・ローテーションのデータ API だけを提供します。
//! GUI は作りません。

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::project::Project;
use crate::project::save::{LoadedProject, SaveError, load_project, save_project};
use crate::settings::AppSettings;

/// オートセーブの1世代分の情報です。復旧一覧に表示する内容です。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutosaveGeneration {
    /// 世代 ID（`<unix ミリ秒>-<連番>`）。辞書順＝作成順です。
    pub id: String,
    /// 紐付くプロジェクトの ID。
    pub project_id: Uuid,
    /// 作成時刻（UNIX ミリ秒）。
    pub created_at_ms: u64,
    /// アーカイブのバイト数。
    pub size_bytes: u64,
    /// アーカイブに含まれるファイル数。
    pub file_count: usize,
}

fn autosave_root(data_dir: &Path) -> PathBuf {
    data_dir.join("autosave")
}

fn project_dir(data_dir: &Path, project_id: Uuid) -> PathBuf {
    autosave_root(data_dir).join(project_id.to_string())
}

fn archive_path(data_dir: &Path, generation: &AutosaveGeneration) -> PathBuf {
    project_dir(data_dir, generation.project_id).join(format!("{}.tar.zst", generation.id))
}

fn sidecar_path(data_dir: &Path, generation: &AutosaveGeneration) -> PathBuf {
    project_dir(data_dir, generation.project_id).join(format!("{}.json", generation.id))
}

/// プロジェクトの現在状態をオートセーブの1世代として書き込みます。
///
/// 書き込み後にローテーション（世代数・全体容量）も行います。
pub fn write_autosave_generation(
    data_dir: &Path,
    project: &Project,
    settings: &AppSettings,
) -> Result<AutosaveGeneration, SaveError> {
    let project_id = project.id();
    let dir = project_dir(data_dir, project_id);
    fs::create_dir_all(&dir)?;

    // プロジェクトを一時フォルダへフォルダ形式で保存し、それを固める。
    let staging = std::env::temp_dir().join(format!("esotereel-autosave-{project_id}"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)?;
    let result = (|| -> Result<Vec<PathBuf>, SaveError> {
        save_project(project, &staging)?;
        let mut files = Vec::new();
        let mut stack = vec![staging.clone()];
        while let Some(current) = stack.pop() {
            for entry in fs::read_dir(&current)? {
                let path = entry?.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    })();
    let files = match result {
        Ok(files) => files,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };

    let created_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut sequence = 0u32;
    let id = loop {
        let id = format!("{created_at_ms:013}-{sequence:04}");
        if !dir.join(format!("{id}.tar.zst")).exists() {
            break id;
        }
        sequence += 1;
    };
    let archive_tmp = dir.join(format!("{id}.tar.zst"));
    let generation = AutosaveGeneration {
        id: id.clone(),
        project_id,
        created_at_ms,
        size_bytes: 0,
        file_count: files.len(),
    };

    // 一時ファイルへ tar+zstd で書き、完成してからリネームする。
    let tmp_path = dir.join(format!("{id}.tar.zst.tmp"));
    let archive_result = (|| -> Result<(), SaveError> {
        let file = fs::File::create(&tmp_path)?;
        let encoder = zstd::stream::write::Encoder::new(file, 0)?;
        let mut tar = tar::Builder::new(encoder);
        for file_path in &files {
            let relative = file_path.strip_prefix(&staging).map_err(|_| {
                SaveError::Invalid("autosave staging path escapes the staging dir".to_string())
            })?;
            tar.append_path_with_name(file_path, relative)
                .map_err(|e| SaveError::Io(std::io::Error::other(e)))?;
        }
        let encoder = tar
            .into_inner()
            .map_err(|e| SaveError::Io(std::io::Error::other(e)))?;
        encoder.finish().map_err(SaveError::from)?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&staging);
    if let Err(error) = archive_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    fs::rename(&tmp_path, &archive_tmp)?;

    let mut generation = generation;
    generation.size_bytes = fs::metadata(&archive_tmp)?.len();
    let sidecar = serde_json::to_string_pretty(&generation)?;
    let sidecar_tmp = sidecar_path(data_dir, &generation).with_extension("json.tmp");
    fs::write(&sidecar_tmp, sidecar)?;
    fs::rename(&sidecar_tmp, sidecar_path(data_dir, &generation))?;

    prune_autosave_generations(data_dir, settings)?;
    Ok(generation)
}

/// オートセーブの世代一覧を返します。新しい順です。
///
/// `project_id` を指定するとそのプロジェクトだけに絞ります。
pub fn list_autosave_generations(
    data_dir: &Path,
    project_id: Option<Uuid>,
) -> Result<Vec<AutosaveGeneration>, SaveError> {
    let root = autosave_root(data_dir);
    let mut generations = Vec::new();
    if !root.exists() {
        return Ok(generations);
    }
    let mut project_dirs = Vec::new();
    if let Some(id) = project_id {
        project_dirs.push(root.join(id.to_string()));
    } else {
        for entry in fs::read_dir(&root)? {
            let path = entry?.path();
            if path.is_dir() {
                project_dirs.push(path);
            }
        }
    }
    for dir in project_dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(generation) = serde_json::from_str::<AutosaveGeneration>(&text) else {
                    log::warn!("ignoring corrupt autosave sidecar {}", path.display());
                    continue;
                };
                generations.push(generation);
            }
        }
    }
    generations.sort_by(|a, b| (b.created_at_ms, &b.id).cmp(&(a.created_at_ms, &a.id)));
    Ok(generations)
}

/// 指定世代を復旧します。アーカイブを一時フォルダへ展開して読み込みます。
///
/// 復旧したプロジェクトをどこへ保存するかは呼び出し側が決めます。
pub fn restore_autosave_generation(
    data_dir: &Path,
    generation_id: &str,
) -> Result<LoadedProject, SaveError> {
    let generation = list_autosave_generations(data_dir, None)?
        .into_iter()
        .find(|generation| generation.id == generation_id)
        .ok_or_else(|| {
            SaveError::Invalid(format!("autosave generation `{generation_id}` not found"))
        })?;
    let staging = std::env::temp_dir().join(format!("esotereel-autosave-restore-{generation_id}"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)?;
    let result = (|| -> Result<LoadedProject, SaveError> {
        let file = fs::File::open(archive_path(data_dir, &generation))?;
        let decoder = zstd::stream::read::Decoder::new(file)?;
        let mut archive = tar::Archive::new(decoder);
        archive
            .unpack(&staging)
            .map_err(|e| SaveError::Io(std::io::Error::other(e)))?;
        load_project(&staging)
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

/// ローテーションを行います。
///
/// プロジェクトごとに最新 N 世代まで残し、全体の容量上限を超えた分は
/// 古い順に削除します。
pub fn prune_autosave_generations(
    data_dir: &Path,
    settings: &AppSettings,
) -> Result<(), SaveError> {
    let generations = list_autosave_generations(data_dir, None)?;
    // 新しい順に並んでいるので、プロジェクトごとに N 件目以降を削除対象にする。
    let mut kept_per_project: std::collections::HashMap<Uuid, usize> =
        std::collections::HashMap::new();
    let mut doomed: Vec<AutosaveGeneration> = Vec::new();
    let mut kept: Vec<AutosaveGeneration> = Vec::new();
    for generation in generations {
        let count = kept_per_project.entry(generation.project_id).or_insert(0);
        if *count < settings.autosave_generations as usize {
            *count += 1;
            kept.push(generation);
        } else {
            doomed.push(generation);
        }
    }
    // 容量上限: 残す世代の合計が上限を超えたら古い順に削る。
    let mut total: u64 = kept.iter().map(|g| g.size_bytes).sum();
    if total > settings.autosave_max_total_bytes {
        // kept は新しい順なので、後ろ（古い方）から削っていく。
        let mut index = kept.len();
        while total > settings.autosave_max_total_bytes && index > 0 {
            index -= 1;
            total -= kept[index].size_bytes;
            doomed.push(kept.remove(index));
        }
    }
    for generation in doomed {
        if let Err(error) = fs::remove_file(archive_path(data_dir, &generation)) {
            log::warn!(
                "autosave: failed to remove pruned generation {}: {error}",
                generation.id
            );
        }
        if let Err(error) = fs::remove_file(sidecar_path(data_dir, &generation)) {
            log::warn!(
                "autosave: failed to remove pruned sidecar {}: {error}",
                generation.id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/project/autosave_tests.rs"]
mod tests;
