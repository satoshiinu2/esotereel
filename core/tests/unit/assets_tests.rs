use super::*;

fn manager_in_temp_dir(tag: &str) -> (UploadManager, PathBuf) {
    let working_dir = std::env::temp_dir().join(format!(
        "esotereel-upload-test-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&working_dir);
    let dirs = Directories::new(None, Some(working_dir.clone()));
    (UploadManager::new(&dirs), working_dir)
}

#[test]
fn begin_rejects_file_names_with_path_separators() {
    let (mut manager, working_dir) = manager_in_temp_dir("traversal");
    let asset_id = AssetRef::new_v4();

    assert!(manager.begin(asset_id, "../evil.mp4", 0, 0).is_err());
    assert!(manager.begin(asset_id, "sub/dir.mp4", 0, 0).is_err());
    assert!(manager.begin(asset_id, "clip.mp4", 0, 0).is_ok());

    let _ = fs::remove_dir_all(&working_dir);
}

#[test]
fn progress_tracks_the_contiguous_prefix_of_out_of_order_chunks() {
    let (mut manager, working_dir) = manager_in_temp_dir("progress");
    let asset_id = AssetRef::new_v4();

    manager.begin(asset_id, "clip.mp4", 5, 0).unwrap();
    let path = working_dir
        .join("data")
        .join("assets")
        .join(asset_id.to_string())
        .join("clip.mp4");
    let progress = manager
        .progress_for_path(&path)
        .expect("an in-progress upload should expose its progress");
    assert_eq!(progress.prefix_len(), 0);

    manager.write_chunk(asset_id, 3, b"de").unwrap();
    assert_eq!(progress.prefix_len(), 0);

    manager.write_chunk(asset_id, 0, b"abc").unwrap();
    assert_eq!(progress.prefix_len(), 5);
    assert!(!progress.is_finished());

    manager.complete(asset_id).unwrap();
    assert!(progress.is_finished());
    assert!(manager.progress_for_path(&path).is_none());

    let _ = fs::remove_dir_all(&working_dir);
}

#[test]
fn complete_requires_the_full_declared_size() {
    let (mut manager, working_dir) = manager_in_temp_dir("size");
    let asset_id = AssetRef::new_v4();

    manager.begin(asset_id, "clip.mp4", 5, 0).unwrap();
    manager.write_chunk(asset_id, 0, b"abc").unwrap();
    assert!(manager.complete(asset_id).is_err());

    manager.write_chunk(asset_id, 3, b"de").unwrap();
    let (name, path, hash) = manager.complete(asset_id).unwrap();
    assert_eq!(name, "clip.mp4");
    assert_eq!(fs::metadata(&path).unwrap().len(), 5);
    assert_eq!(hash, *blake3::hash(b"abcde").as_bytes());

    let _ = fs::remove_dir_all(&working_dir);
}

/// 順不同の受信でも、完了時のハッシュはファイル内容と一致すること。
///
/// 後ろのチャンクが先に届くと受信時の incremental 計算は使えず、
/// 完了時にファイルを読み直して計算するフォールバックに入る。
#[test]
fn complete_hashes_out_of_order_upload_correctly() {
    let (mut manager, working_dir) = manager_in_temp_dir("hash-ooo");
    let asset_id = AssetRef::new_v4();

    manager.begin(asset_id, "clip.mp4", 5, 0).unwrap();
    manager.write_chunk(asset_id, 3, b"de").unwrap();
    manager.write_chunk(asset_id, 0, b"abc").unwrap();
    let (_, _, hash) = manager.complete(asset_id).unwrap();
    assert_eq!(hash, *blake3::hash(b"abcde").as_bytes());

    let _ = fs::remove_dir_all(&working_dir);
}
