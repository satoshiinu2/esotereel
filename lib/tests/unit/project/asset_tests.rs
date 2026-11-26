use super::*;

#[test]
fn get_or_add_external_deduplicates_by_path() {
    let mut registry = AssetRegistry::default();
    let path = Path::new("/tmp/video.mp4");

    let id1 = registry.get_or_add_external(path).unwrap();
    let id2 = registry.get_or_add_external(path).unwrap();
    assert_eq!(id1, id2, "同一パスは同じ AssetRef を返すべき");
    assert_eq!(registry.iter().count(), 1, "重複登録されない");
}

#[test]
fn get_or_add_external_resolve_roundtrip() {
    let mut registry = AssetRegistry::default();
    let path = Path::new("/tmp/video.mp4");
    let id = registry.get_or_add_external(path).unwrap();

    let resolved = registry.resolve(id).unwrap();
    assert_eq!(resolved, path);
}

#[test]
fn resolve_by_path_returns_none_for_unknown_path() {
    let registry = AssetRegistry::default();
    assert!(
        registry
            .resolve_by_path(Path::new("/tmp/unknown.mp4"))
            .is_none()
    );
}

#[test]
fn resolve_by_path_returns_id_for_registered_path() {
    let mut registry = AssetRegistry::default();
    let path = Path::new("/tmp/video.mp4");
    let id = registry.get_or_add_external(path).unwrap();
    assert_eq!(registry.resolve_by_path(path), Some(id));
}

/// 登録時に実在ファイルのサイズが fingerprint に記録されること。
/// 存在しないパスの登録は成功し、fingerprint は None のままになること。
#[test]
fn registration_records_file_size_in_fingerprint() {
    let path = std::env::temp_dir().join(format!(
        "esotereel-fingerprint-test-{}.mp4",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&path, vec![0u8; 1234]).unwrap();

    let mut registry = AssetRegistry::default();
    let id = registry.get_or_add_external(&path).unwrap();
    let entry = registry.get(id).unwrap();
    assert_eq!(
        entry.fingerprint.as_ref().map(|f| f.size),
        Some(1234),
        "登録時に stat したサイズが残るべき"
    );
    assert_eq!(
        entry.fingerprint.as_ref().and_then(|f| f.hash),
        None,
        "hash は登録時には計算しない"
    );

    let missing = Path::new("/tmp/esotereel-no-such-file.mp4");
    let missing_id = registry.get_or_add_external(missing).unwrap();
    assert_eq!(
        registry.get(missing_id).unwrap().fingerprint,
        None,
        "存在しないパスは fingerprint なしで登録できるべき"
    );

    std::fs::remove_file(&path).unwrap();
}
