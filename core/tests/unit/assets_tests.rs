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
    let asset_id = AssetId::new_v4();

    assert!(manager.begin(asset_id, "../evil.mp4", 0).is_err());
    assert!(manager.begin(asset_id, "sub/dir.mp4", 0).is_err());
    assert!(manager.begin(asset_id, "clip.mp4", 0).is_ok());

    let _ = fs::remove_dir_all(&working_dir);
}

#[test]
fn complete_requires_the_full_declared_size() {
    let (mut manager, working_dir) = manager_in_temp_dir("size");
    let asset_id = AssetId::new_v4();

    manager.begin(asset_id, "clip.mp4", 5).unwrap();
    manager.write_chunk(asset_id, 0, b"abc").unwrap();
    assert!(manager.complete(asset_id).is_err());

    manager.write_chunk(asset_id, 3, b"de").unwrap();
    let (name, path) = manager.complete(asset_id).unwrap();
    assert_eq!(name, "clip.mp4");
    assert_eq!(fs::metadata(&path).unwrap().len(), 5);

    let _ = fs::remove_dir_all(&working_dir);
}
