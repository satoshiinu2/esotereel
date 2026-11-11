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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::plugin::NamespacedID;
    use crate::plugin::property::value::FieldValue;
    use crate::project::asset::{AssetEntry, AssetAvailability, AssetFingerprint};
    use crate::project::clip::ClipBindingValue;

    /// テスト用の一時ディレクトリを作成し、そのパスを返します。
    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "esotereel-save-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// タイムライン・フォルダー・クリップ（プロパティ付き）・アセットを
    /// 含む代表的なプロジェクトを構築します。タイムライン ID も返します。
    fn sample_project() -> (Project, crate::project::ids::TimelineId) {
        let mut project = Project::new();
        let timeline_id = project.insert_timeline(30.0);

        // フォルダーと、その中へのネストしたレイヤーでアウトラインを厚くする。
        let folder_id = project
            .insert_folder_in_timeline(timeline_id, None, None, "Folder".to_string())
            .unwrap();
        project
            .insert_layer_in_timeline(
                timeline_id,
                Some(folder_id),
                None,
                "Nested".to_string(),
            )
            .unwrap();

        // デフォルトレイヤーの先頭に、プロパティ付きクリップを置く。
        let layer_id = *project
            .timeline_ref(timeline_id)
            .unwrap()
            .iter_layers()
            .next()
            .unwrap()
            .0;
        let mut properties = HashMap::new();
        properties.insert(
            NamespacedID::new("std", "source").unwrap(),
            ClipBindingValue::Static(FieldValue::String("/media/clip1.mp4".to_string())),
        );
        project
            .new_clip_in_timeline(
                timeline_id,
                layer_id,
                0,
                90,
                NamespacedID::new("std", "video").unwrap(),
                properties,
            )
            .unwrap();

        // アセット: 通常の外部ファイルと、フィンガープリント付きの Missing エントリ。
        project
            .register_asset(Path::new("/media/clip1.mp4"))
            .unwrap();
        project
            .assets_mut()
            .add(AssetEntry {
                id: uuid::Uuid::new_v4(),
                name: "gone.mp4".to_string(),
                source: AssetAvailability::Missing {
                    previous_path: PathBuf::from("/media/gone.mp4"),
                },
                project_path: Some(PathBuf::from("assets/gone.mp4")),
                fingerprint: Some(AssetFingerprint {
                    size: 4096,
                    hash: Some([7u8; 32]),
                }),
            })
            .unwrap();

        (project, timeline_id)
    }

    #[test]
    fn save_load_round_trip_preserves_project() {
        let dir = test_dir();
        let path = dir.join("project.json");
        let (project, timeline_id) = sample_project();

        save_project(&project, &path).unwrap();
        assert!(path.exists());
        // 一時ファイルはリネームで消費され、残らない。
        assert!(!tmp_path_for(&path).exists());

        let loaded = load_project(&path).unwrap();

        // Project には PartialEq がなく、Clip の PartialEq も ID のみ比較のため、
        // 最も強い比較として serde 値同士を丸ごと比べる。
        assert_eq!(
            serde_json::to_value(&project).unwrap(),
            serde_json::to_value(&loaded).unwrap(),
            "保存→読み込みで内容が変わっている"
        );

        // 構造面のスポットチェック。
        assert_eq!(loaded.timeline_count(), 1);
        let timeline = loaded.timeline_ref(timeline_id).unwrap();
        assert_eq!(timeline.iter_clips().count(), 1);
        assert_eq!(loaded.assets().iter().count(), 2);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn loaded_project_continues_id_allocation_without_collision() {
        let dir = test_dir();
        let path = dir.join("project.json");
        let (project, existing_id) = sample_project();

        save_project(&project, &path).unwrap();
        let mut loaded = load_project(&path).unwrap();

        // IdGenerator のカウンタが復元され、新規 ID が既存と衝突しない。
        let new_id = loaded.insert_timeline(24.0);
        assert_ne!(new_id, existing_id);
        assert_eq!(loaded.timeline_count(), 2);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_rejects_unsupported_version() {
        let dir = test_dir();
        let path = dir.join("future.json");
        fs::write(&path, r#"{ "version": 999, "project": {} }"#).unwrap();

        let err = load_project(&path).unwrap_err();
        assert!(
            matches!(
                err,
                SaveError::UnsupportedVersion {
                    found: 999,
                    supported: PROJECT_FILE_VERSION
                }
            ),
            "unexpected error: {err:?}"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_missing_file_is_io_error() {
        let path = std::env::temp_dir().join(format!(
            "esotereel-no-such-file-{}.json",
            uuid::Uuid::new_v4()
        ));
        let err = load_project(&path).unwrap_err();
        assert!(
            matches!(err, SaveError::Io(ref e) if e.kind() == std::io::ErrorKind::NotFound),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn load_corrupt_file_is_format_error() {
        let dir = test_dir();
        let path = dir.join("corrupt.json");
        fs::write(&path, "this is not json").unwrap();

        let err = load_project(&path).unwrap_err();
        assert!(
            matches!(err, SaveError::Format(_)),
            "unexpected error: {err:?}"
        );

        fs::remove_dir_all(&dir).unwrap();
    }
}
