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
