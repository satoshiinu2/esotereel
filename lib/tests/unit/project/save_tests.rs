use super::*;
use std::collections::HashMap;

use crate::plugin::NamespacedID;
use crate::plugin::property::PropertyKey;
use crate::plugin::property::value::FieldValue;
use crate::project::asset::{AssetAvailability, AssetEntry, AssetFingerprint};
use crate::project::clip::{ClipBindingValue, ShaderStackEntry};

/// テスト用の一時ディレクトリを作成し、そのパスを返します。
fn test_dir() -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("esotereel-save-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 現行モデル（std:media / std:transform のコンポーネント分割、shader_stack、
/// External / Managed / Missing のアセット）を持つサンプルプロジェクトを構築します。
///
/// クリップはチャンクをまたぐ配置にし、開始位置が属するチャンクへの
/// 所有規則（チャンク 0: 0〜29_999、チャンク 1: 30_000〜）も検証できるようにします。
fn sample_project() -> (Project, crate::project::ids::TimelineId) {
    let mut project = Project::new();
    let timeline_id = project.insert_timeline(30.0);

    let folder_id = project
        .insert_folder_in_timeline(timeline_id, None, None, "Folder".to_string())
        .unwrap();
    let nested_layer_id = project
        .insert_layer_in_timeline(
            timeline_id,
            Some(folder_id),
            None,
            "Nested".to_string(),
        )
        .unwrap();
    let first_layer_id = *project
        .timeline_ref(timeline_id)
        .unwrap()
        .iter_layers()
        .find(|(_, layer)| layer.name == "Layer 1")
        .unwrap()
        .0;

    let mut properties = HashMap::new();
    properties.insert(
        PropertyKey::parse("std:media#media").unwrap(),
        ClipBindingValue::Static(FieldValue::Map(
            [
                (
                    "source".to_string(),
                    FieldValue::String("/media/clip1.mp4".to_string()),
                ),
                (
                    "source_offset".to_string(),
                    FieldValue::String("0".to_string()),
                ),
            ]
            .into_iter()
            .collect(),
        )),
    );
    properties.insert(
        PropertyKey::parse("std:transform#transform").unwrap(),
        ClipBindingValue::Static(FieldValue::Map(
            [
                (
                    "position".to_string(),
                    FieldValue::Array(vec![
                        FieldValue::Float(0.0),
                        FieldValue::Float(0.0),
                        FieldValue::Float(0.0),
                    ]),
                ),
                (
                    "scale".to_string(),
                    FieldValue::Array(vec![
                        FieldValue::Float(1.0),
                        FieldValue::Float(1.0),
                        FieldValue::Float(1.0),
                    ]),
                ),
            ]
            .into_iter()
            .collect(),
        )),
    );
    let clip_id = project
        .new_clip_in_timeline(
            timeline_id,
            first_layer_id,
            0,
            90,
            NamespacedID::new("std", "video").unwrap(),
            properties,
        )
        .unwrap();
    project
        .timeline_mut(timeline_id)
        .unwrap()
        .get_clip_mut(clip_id)
        .unwrap()
        .set_shader_stack(vec![ShaderStackEntry {
            shader_id: NamespacedID::new("std", "wave_compute").unwrap(),
            properties: [("amplitude".to_string(), FieldValue::Float(10.0))]
                .into_iter()
                .collect(),
        }]);

    // チャンク 0 に開始しチャンク 1 へはみ出す長いクリップ（所有は開始位置で決まる）。
    project
        .new_clip_in_timeline(
            timeline_id,
            first_layer_id,
            29_000,
            5_000,
            NamespacedID::new("std", "video").unwrap(),
            HashMap::new(),
        )
        .unwrap();
    // チャンク 1 に開始するクリップ。
    project
        .new_clip_in_timeline(
            timeline_id,
            nested_layer_id,
            35_000,
            120,
            NamespacedID::new("std", "video").unwrap(),
            HashMap::new(),
        )
        .unwrap();

    project
        .register_asset(Path::new("/media/clip1.mp4"))
        .unwrap();
    project
        .register_managed_asset(
            uuid::Uuid::new_v4(),
            "managed.mp4".to_string(),
            PathBuf::from("/data/assets/managed.mp4"),
        )
        .unwrap();
    project
        .assets_mut()
        .add(AssetEntry {
            id: uuid::Uuid::new_v4(),
            name: "gone.mp4".to_string(),
            source: AssetAvailability::Missing {
                previous_path: PathBuf::from("/media/gone.mp4"),
            },
            fingerprint: Some(AssetFingerprint {
                size: 4096,
                hash: Some([7u8; 32]),
            }),
        })
        .unwrap();

    (project, timeline_id)
}

/// フォルダ内の全ファイルを相対パス → パース済み JSON 値で集めます。
fn read_tree(dir: &Path) -> BTreeMap<PathBuf, serde_json::Value> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path.strip_prefix(dir).unwrap().to_path_buf();
                let text = fs::read_to_string(&path).unwrap();
                out.insert(
                    relative,
                    serde_json::from_str(&text).unwrap_or_else(|_| {
                        panic!("{} is not valid JSON", path.display())
                    }),
                );
            }
        }
    }
    out
}

/// フォルダ内に一時ファイル（*.tmp）が残っていないことを確認します。
fn assert_no_tmp_files(dir: &Path) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                assert!(
                    !path.extension().is_some_and(|ext| ext == "tmp"),
                    "temporary file left behind: {}",
                    path.display()
                );
            }
        }
    }
}

/// save → load → save のラウンドトリップでプロジェクト内容が等価になること。
///
/// 本機能の中核の観測可能な契約です。`Project` に `PartialEq` がないため、
/// 最も強い比較として読み込み直後に再保存したフォルダ全体を
/// パース済み JSON 値同士で比べます（キー順の違いは吸収されます）。
#[test]
fn save_load_round_trip_preserves_project() {
    let dir = test_dir();
    let (project, timeline_id) = sample_project();
    let project_id = project.id();

    save_project(&project, &dir).unwrap();
    assert!(dir.join("project.json").exists());
    assert_no_tmp_files(&dir);

    let loaded = load_project(&dir).unwrap();
    assert_eq!(loaded.version, CURRENT_PROJECT_VERSION);
    assert_eq!(loaded.status, VersionStatus::Compatible);
    assert_eq!(loaded.project.id(), project_id);
    assert_eq!(loaded.project.timeline_count(), 1);
    assert_eq!(loaded.project.assets().iter().count(), 3);
    let timeline = loaded.project.timeline_ref(timeline_id).unwrap();
    assert_eq!(timeline.iter_clips().count(), 3);

    // チャンクマニフェスト: 開始位置で 0 / 1 に分かれ、max_end は
    // チャンク 0 内の長いクリップの終了位置（29_000 + 5_000）になる。
    let timeline_text =
        fs::read_to_string(dir.join("timelines").join(timeline_id.to_string()).join("timeline.json"))
            .unwrap();
    let timeline_json: serde_json::Value = serde_json::from_str(&timeline_text).unwrap();
    let chunks = timeline_json["chunks"].as_array().unwrap();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0]["index"], 0);
    assert_eq!(chunks[0]["max_end"], 34_000);
    assert_eq!(chunks[1]["index"], 1);
    assert_eq!(chunks[1]["max_end"], 35_120);

    // 再保存したフォルダ全体が、保存直後のフォルダと等価になる。
    let dir2 = test_dir();
    save_project(&loaded.project, &dir2).unwrap();
    assert_eq!(
        read_tree(&dir),
        read_tree(&dir2),
        "保存→読み込み→再保存で内容が変わっている"
    );

    // IdGenerator が復元され、新規 ID が既存と衝突しない。
    let mut reloaded = loaded.project;
    let new_id = reloaded.insert_timeline(24.0);
    assert_ne!(new_id, timeline_id);
    assert_eq!(reloaded.timeline_count(), 2);

    fs::remove_dir_all(&dir).unwrap();
    fs::remove_dir_all(&dir2).unwrap();
}

/// ファイルのバージョンに対する判定が、段階（revision / minor / major）に
/// 応じた `VersionStatus` を返すこと。対応外の major 0 は見える失敗になる。
#[test]
fn load_reports_version_status_for_newer_files() {
    let dir = test_dir();
    let write_project = |version: &str| {
        fs::write(
            dir.join("project.json"),
            format!(
                r#"{{"version": {version}, "id": "00000000-0000-0000-0000-000000000000", "timelines": [], "assets": {{"entries": {{}}, "order": []}}}}"#
            ),
        )
        .unwrap();
    };

    write_project(r#"{"major": 1, "minor": 0, "revision": 5}"#);
    assert_eq!(
        load_project(&dir).unwrap().status,
        VersionStatus::NewerRevision
    );

    write_project(r#"{"major": 1, "minor": 1, "revision": 0}"#);
    assert_eq!(
        load_project(&dir).unwrap().status,
        VersionStatus::NewerMinor
    );

    write_project(r#"{"major": 2, "minor": 0, "revision": 0}"#);
    assert_eq!(
        load_project(&dir).unwrap().status,
        VersionStatus::NewerMajor
    );

    write_project(r#"{"major": 0, "minor": 9, "revision": 0}"#);
    assert!(
        matches!(
            load_project(&dir).unwrap_err(),
            SaveError::UnsupportedVersion { .. }
        ),
        "major 0 は対応外として拒否されるべき"
    );

    fs::write(dir.join("project.json"), "this is not json").unwrap();
    assert!(
        matches!(load_project(&dir).unwrap_err(), SaveError::Format(_)),
        "壊れた project.json は Format エラーになるべき"
    );

    let err = load_project(&std::env::temp_dir().join(format!(
        "esotereel-no-such-project-{}",
        uuid::Uuid::new_v4()
    )))
    .unwrap_err();
    assert!(
        matches!(err, SaveError::Io(ref e) if e.kind() == std::io::ErrorKind::NotFound),
        "存在しないフォルダは Io エラーになるべき"
    );

    fs::remove_dir_all(&dir).unwrap();
}

/// overview.json は再生成可能なキャッシュなので、欠落・破損・実体との
/// 不整合があっても、チャンク実体から配置を再生成して開けること。
/// 再生成の正しさは「再保存したフォルダが元の保存と等価」で検証します。
#[test]
fn load_regenerates_overview_from_chunks() {
    let pristine = test_dir();
    let (project, timeline_id) = sample_project();
    save_project(&project, &pristine).unwrap();
    let overview_path = |dir: &Path| {
        dir.join("timelines")
            .join(timeline_id.to_string())
            .join("overview.json")
    };

    // 欠落・破損・不整合の3パターンをそれぞれ壊してから読み直す。
    for damage in ["missing", "corrupt", "stale"] {
        let dir = test_dir();
        save_project(&project, &dir).unwrap();
        match damage {
            "missing" => fs::remove_file(overview_path(&dir)).unwrap(),
            "corrupt" => fs::write(overview_path(&dir), "not json").unwrap(),
            _ => {
                let text = fs::read_to_string(overview_path(&dir)).unwrap();
                let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
                json["placements"][0]["position"] = serde_json::json!(123_456);
                fs::write(overview_path(&dir), json.to_string()).unwrap();
            }
        }

        let loaded = load_project(&dir)
            .unwrap_or_else(|e| panic!("overview が {damage} でも開けるべき: {e}"));
        let resaved = test_dir();
        save_project(&loaded.project, &resaved).unwrap();
        assert_eq!(
            read_tree(&pristine),
            read_tree(&resaved),
            "overview が {damage} の場合の再生成結果が元の保存と異なる"
        );
        fs::remove_dir_all(&dir).unwrap();
        fs::remove_dir_all(&resaved).unwrap();
    }

    fs::remove_dir_all(&pristine).unwrap();
}

/// フォルダ形式の ChunkStore を付けたタイムラインは、チャンクを
/// アンロードしても配置が残り、読み戻しで実体が復元すること。
/// アンロード済みを含む独立化（deep_clone）も実体を取りこぼさないこと。
#[test]
fn folder_chunk_store_supports_unload_reload_and_independent_copy() {
    use crate::project::chunk::ChunkState;

    let dir = test_dir();
    let (project, timeline_id) = sample_project();
    save_project(&project, &dir).unwrap();

    let mut loaded = load_project(&dir).unwrap().project;
    loaded.attach_folder_chunk_stores(&dir);
    let timeline = loaded.timeline_mut(timeline_id).unwrap();
    let chunk0_clip = timeline
        .iter_placements()
        .find(|(_, placement)| placement.position < 30_000)
        .map(|(clip_id, _)| clip_id)
        .unwrap();

    timeline.unload_chunk(0).unwrap();
    assert_eq!(timeline.chunk_state(0), Some(ChunkState::Unloaded));
    assert!(timeline.get_clip(chunk0_clip).is_none());
    assert!(timeline.get_placement(chunk0_clip).is_some());

    // アンロード済みを含む独立化は、先に読み込んでから複製される。
    let copy_id = loaded.make_independent(timeline_id).unwrap();
    assert_eq!(
        loaded.timeline_ref(copy_id).unwrap().iter_clips().count(),
        3
    );

    let timeline = loaded.timeline_mut(timeline_id).unwrap();
    timeline.ensure_all_chunks_loaded().unwrap();
    assert_eq!(timeline.chunk_state(0), Some(ChunkState::Loaded));
    assert_eq!(timeline.iter_clips().count(), 3);

    fs::remove_dir_all(&dir).unwrap();
}
