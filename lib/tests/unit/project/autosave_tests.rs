use super::*;
use std::collections::HashMap;

use crate::plugin::NamespacedID;
use crate::project::clip::ClipBindingValue;
use crate::settings::AppSettings;

fn test_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("esotereel-autosave-test-{}-{tag}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn sample_project() -> Project {
    let mut project = Project::new();
    let timeline_id = project.insert_timeline(30.0);
    let layer_id = *project
        .timeline_ref(timeline_id)
        .unwrap()
        .iter_layers()
        .next()
        .unwrap()
        .0;
    project
        .new_clip_in_timeline(
            timeline_id,
            layer_id,
            0,
            90,
            NamespacedID::new("std", "video").unwrap(),
            HashMap::<
                crate::plugin::property::PropertyKey,
                ClipBindingValue,
            >::new(),
        )
        .unwrap();
    project
        .register_asset(std::path::Path::new("/media/clip1.mp4"))
        .unwrap();
    project
}

/// フォルダ内の全ファイルを相対パス → パース済み JSON 値で集めます。
fn read_tree(dir: &Path) -> std::collections::BTreeMap<PathBuf, serde_json::Value> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path.strip_prefix(dir).unwrap().to_path_buf();
                let text = fs::read_to_string(&path).unwrap();
                out.insert(relative, serde_json::from_str(&text).unwrap());
            }
        }
    }
    out
}

/// 書き込み → 一覧 → 復旧のラウンドトリップで内容が等価になること。
#[test]
fn write_list_restore_round_trip_preserves_project() {
    let dir = test_dir("roundtrip");
    let data_dir = dir.join("data");
    let project = sample_project();
    let project_id = project.id();
    let settings = AppSettings::default();

    let generation = write_autosave_generation(&data_dir, &project, &settings).unwrap();
    assert_eq!(generation.project_id, project_id);
    assert!(generation.size_bytes > 0);
    assert!(generation.file_count >= 4, "project.json + 3 timeline files");

    let listed = list_autosave_generations(&data_dir, Some(project_id)).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, generation.id);

    let restored = restore_autosave_generation(&data_dir, &generation.id).unwrap();
    assert_eq!(restored.project.id(), project_id);
    assert_eq!(restored.status, crate::project::save::VersionStatus::Compatible);

    // 復旧したプロジェクトを再保存したフォルダが、元の保存と等価になる。
    let dir_a = dir.join("a");
    let dir_b = dir.join("b");
    save_project(&project, &dir_a).unwrap();
    save_project(&restored.project, &dir_b).unwrap();
    assert_eq!(read_tree(&dir_a), read_tree(&dir_b));

    fs::remove_dir_all(&dir).unwrap();
}

/// 世代数の上限で古い世代からローテーションされること。
#[test]
fn generation_count_limit_prunes_oldest_first() {
    let dir = test_dir("generations");
    let data_dir = dir.join("data");
    let project = sample_project();
    let settings = AppSettings {
        autosave_generations: 2,
        ..AppSettings::default()
    };

    let first = write_autosave_generation(&data_dir, &project, &settings).unwrap();
    let second = write_autosave_generation(&data_dir, &project, &settings).unwrap();
    let third = write_autosave_generation(&data_dir, &project, &settings).unwrap();

    let listed = list_autosave_generations(&data_dir, Some(project.id())).unwrap();
    assert_eq!(listed.len(), 2, "最新 2 世代だけ残るべき");
    assert_eq!(listed[0].id, third.id);
    assert_eq!(listed[1].id, second.id);
    assert!(
        !data_dir
            .join("autosave")
            .join(project.id().to_string())
            .join(format!("{}.tar.zst", first.id))
            .exists(),
        "最も古い世代のアーカイブは消えているべき"
    );

    fs::remove_dir_all(&dir).unwrap();
}

/// 全体容量の上限で古い世代から削られること。
#[test]
fn total_capacity_limit_prunes_oldest_first() {
    let dir = test_dir("capacity");
    let data_dir = dir.join("data");
    let project_a = sample_project();
    let project_b = sample_project();
    let defaults = AppSettings::default();

    let gen_a = write_autosave_generation(&data_dir, &project_a, &defaults).unwrap();
    let gen_b = write_autosave_generation(&data_dir, &project_b, &defaults).unwrap();

    // 上限を合計より 1 バイトだけ小さくすると、最も古い世代だけ削られる。
    let settings = AppSettings {
        autosave_generations: 10,
        autosave_max_total_bytes: gen_a.size_bytes + gen_b.size_bytes - 1,
        ..AppSettings::default()
    };
    prune_autosave_generations(&data_dir, &settings).unwrap();
    let listed = list_autosave_generations(&data_dir, None).unwrap();
    assert_eq!(listed.len(), 1, "上限を超えた分だけ削られるべき");
    assert_eq!(listed[0].id, gen_b.id);

    // 容量上限が 0 なら、世代はすべて削られる。
    let tiny = AppSettings {
        autosave_generations: 10,
        autosave_max_total_bytes: 0,
        ..AppSettings::default()
    };
    prune_autosave_generations(&data_dir, &tiny).unwrap();
    let listed = list_autosave_generations(&data_dir, None).unwrap();
    assert_eq!(listed.len(), 0, "上限 0 では世代は残らない");

    fs::remove_dir_all(&dir).unwrap();
}

/// 存在しない世代の復旧は見える失敗になること。
#[test]
fn restore_of_unknown_generation_fails() {
    let dir = test_dir("unknown");
    let data_dir = dir.join("data");
    let err = restore_autosave_generation(&data_dir, "20260101-0000-0000")
        .unwrap_err();
    assert!(
        matches!(err, SaveError::Invalid(_)),
        "unknown generation should fail visibly: {err:?}"
    );
    fs::remove_dir_all(&dir).unwrap();
}
