//! プロジェクトの保存・読み込みです。
//!
//! ファイル形式は JSON で、次のエンベロープ構造を持ちます。
//!
//! ```json
//! { "version": 1, "project": { /* Project の serde 表現 */ } }
//! ```
//!
//! - `version` はこのファイル形式自体のバージョンです。対応していない
//!   バージョンのファイルは、パニックではなく `SaveError::UnsupportedVersion`
//!   で拒否します。
//! - アセットのメディアパスは、登録されている絶対パスのまま保存します。
//!   プロジェクトファイルの場所を基準とした相対化は行いません（将来課題）。
//! - `Timeline` の実行時状態（`chunk_index` キャッシュと未同期の `changes`）は
//!   `#[serde(skip)]` のため保存されず、読み込み時にデフォルト値で再構築されます。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::Project;

/// プロジェクトファイル形式の現行バージョンです。
pub const PROJECT_FILE_VERSION: u32 = 1;

/// プロジェクトの保存・読み込みで発生するエラーです。
#[derive(Debug, Error)]
pub enum SaveError {
    /// ファイルの読み書きに失敗した（ファイルが存在しない場合を含む）。
    #[error("project file I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON として壊れている、または必須フィールドが欠けている。
    #[error("project file format error: {0}")]
    Format(#[from] serde_json::Error),
    /// このビルドが対応していないファイルバージョンだった。
    #[error("unsupported project file version: {found} (supported: {supported})")]
    UnsupportedVersion { found: u32, supported: u32 },
}

/// ファイルのエンベロープです。バージョン判定を `Project` 本体の
/// デシリアライズより先に行うため、ヘッダーだけの型も別に持ちます。
/// 保存側は `Project` を借用のまま直列化します（`Project` は `Clone` でないため）。
#[derive(Deserialize)]
struct ProjectFile {
    version: u32,
    project: Project,
}

#[derive(Serialize)]
struct ProjectFileRef<'a> {
    version: u32,
    project: &'a Project,
}

#[derive(Deserialize)]
struct ProjectFileHeader {
    version: u32,
}

/// プロジェクトを `path` へ JSON 形式で保存します。
///
/// 書き込みは同一ディレクトリ内の一時ファイル経由で行い、書き終えてから
/// 本来のパスへリネームするため、途中で失敗しても既存ファイルは壊れません。
pub fn save_project(project: &Project, path: &Path) -> Result<(), SaveError> {
    let file = ProjectFileRef {
        version: PROJECT_FILE_VERSION,
        project,
    };
    let text = serde_json::to_string_pretty(&file)?;

    let tmp_path = tmp_path_for(path);
    fs::write(&tmp_path, text)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// `path` からプロジェクトを読み込みます。
///
/// バージョンが対応範囲外の場合は `SaveError::UnsupportedVersion` を返します。
pub fn load_project(path: &Path) -> Result<Project, SaveError> {
    let text = fs::read_to_string(path)?;

    // バージョンを先に確認する。将来形式が変わっても、古い読み込み側が
    // 本体パースの失敗ではなくバージョンエラーで報告できるようにするため。
    let header: ProjectFileHeader = serde_json::from_str(&text)?;
    ensure_supported_version(header.version)?;

    let file: ProjectFile = serde_json::from_str(&text)?;
    ensure_supported_version(file.version)?;
    Ok(file.project)
}

/// バージョンが対応範囲内（`1..=PROJECT_FILE_VERSION`）かを確認します。
fn ensure_supported_version(found: u32) -> Result<(), SaveError> {
    if !(1..=PROJECT_FILE_VERSION).contains(&found) {
        return Err(SaveError::UnsupportedVersion {
            found,
            supported: PROJECT_FILE_VERSION,
        });
    }
    Ok(())
}

/// 保存用の一時ファイルパス（対象と同一ディレクトリ、`*.tmp` 付け替え）です。
fn tmp_path_for(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}
