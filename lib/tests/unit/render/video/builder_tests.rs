use super::*;

#[test]
fn resolve_video_source_resolves_asset_id_through_registry() {
    let project = Arc::new(RwLock::new(Some(Project::new())));
    let id = project
        .write()
        .unwrap()
        .as_mut()
        .unwrap()
        .register_asset(Path::new("/tmp/video.mp4"))
        .unwrap();

    // UUID（アセットID）形式 → アセットレジストリ経由で実パスに解決
    assert_eq!(
        resolve_video_source(&id.to_string(), &project),
        Some("/tmp/video.mp4".to_string())
    );
}

#[test]
fn resolve_video_source_passes_through_raw_path() {
    let project = Arc::new(RwLock::new(Some(Project::new())));
    // 生パス形式 → そのまま返す（従来と同等の動作）
    assert_eq!(
        resolve_video_source("/tmp/video.mp4", &project),
        Some("/tmp/video.mp4".to_string())
    );
}

#[test]
fn resolve_video_source_skips_unregistered_asset_id() {
    let project = Arc::new(RwLock::new(Some(Project::new())));
    let unknown = Uuid::new_v4();
    // 未登録のアセットIDは None を返し、存在しないパスを初期化しない
    assert_eq!(resolve_video_source(&unknown.to_string(), &project), None);
}

#[test]
fn resolve_video_source_works_without_project() {
    let project = Arc::new(RwLock::new(None));
    // プロジェクトが None でも生パスはそのまま返す
    assert_eq!(
        resolve_video_source("/tmp/video.mp4", &project),
        Some("/tmp/video.mp4".to_string())
    );
    assert_eq!(
        resolve_video_source(&Uuid::new_v4().to_string(), &project),
        None
    );
}
