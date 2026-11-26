use std::path::Path;

use uuid::Uuid;

use super::*;

#[test]
fn resolve_video_source_resolves_asset_ref_through_registry() {
    let project = Arc::new(RwLock::new(Some(Project::new())));
    let id = project
        .write()
        .unwrap()
        .as_mut()
        .unwrap()
        .register_asset(Path::new("/tmp/video.mp4"))
        .unwrap();

    // アセット参照 → アセットレジストリ経由で実パスに解決
    assert_eq!(
        resolve_video_source(id, &project),
        Some("/tmp/video.mp4".to_string())
    );
}

#[test]
fn resolve_video_source_skips_unregistered_asset_ref() {
    let project = Arc::new(RwLock::new(Some(Project::new())));
    let unknown = Uuid::new_v4();
    // 未登録のアセット参照は None を返し、存在しないパスを初期化しない
    assert_eq!(resolve_video_source(unknown, &project), None);
}

#[test]
fn resolve_video_source_works_without_project() {
    let project = Arc::new(RwLock::new(None));
    // プロジェクトが無ければ参照は解決できない
    assert_eq!(resolve_video_source(Uuid::new_v4(), &project), None);
}
