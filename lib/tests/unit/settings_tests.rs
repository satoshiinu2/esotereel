use super::*;
use std::fs;
use std::path::PathBuf;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn test_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "esotereel-settings-test-{}-{tag}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 既定値はドキュメント通りの値になること。
#[test]
fn defaults_match_documented_values() {
    let settings = AppSettings::default();
    assert_eq!(settings.chunk_default_width_ticks, CHUNK_TICKS);
    assert_eq!(settings.chunk_width_ticks, None);
    assert_eq!(settings.chunk_unload_after_secs, 600);
    assert!(settings.chunk_keep_frequently_used);
    assert_eq!(settings.autosave_interval_secs, 300);
    assert_eq!(settings.autosave_generations, 5);
    assert_eq!(settings.autosave_max_total_bytes, 1024 * 1024 * 1024);
}

/// 階層解決: 既定値＜ユーザー＜プロジェクト＜サーバーで後の層が勝つこと。
#[test]
fn later_layers_override_earlier_ones() {
    let dir = test_dir("layers");
    let user = dir.join("user.toml");
    let project_dir = dir.join("project");
    let server = dir.join("server.toml");
    write(
        &user,
        "[chunk]\nunload_after_secs = 100\n[autosave]\ngenerations = 2\n",
    );
    write(
        &project_dir.join("settings.toml"),
        "[chunk]\nunload_after_secs = 200\n",
    );
    write(&server, "[chunk]\nunload_after_secs = 300\n");

    let settings =
        AppSettings::load(Some(&user), Some(&project_dir), Some(&server)).unwrap();
    // Overridable なキーはサーバーが最終的に勝つ。
    assert_eq!(settings.chunk_unload_after_secs, 300);
    // ユーザーだけが指定したキーはユーザーの値が残る。
    assert_eq!(settings.autosave_generations, 2);

    fs::remove_dir_all(&dir).unwrap();
}

/// スコープ外の層にある値は無視されること。
///
/// `chunk.default_width_ticks` はユーザーオンリーなのでプロジェクト側の
/// 指定は無視され、`chunk.width_ticks` はプロジェクト／サーバーオンリー
/// なのでユーザー側の指定は無視される。
#[test]
fn values_outside_declared_scope_are_ignored() {
    let dir = test_dir("scope");
    let user = dir.join("user.toml");
    let project_dir = dir.join("project");
    write(&user, "[chunk]\nwidth_ticks = 60000\n");
    write(
        &project_dir.join("settings.toml"),
        "[chunk]\ndefault_width_ticks = 1000\nwidth_ticks = 45000\n",
    );

    let settings = AppSettings::load(Some(&user), Some(&project_dir), None).unwrap();
    assert_eq!(
        settings.chunk_default_width_ticks, CHUNK_TICKS,
        "プロジェクト側の default_width_ticks は無視されるべき"
    );
    assert_eq!(
        settings.chunk_width_ticks,
        Some(45000),
        "プロジェクト側の width_ticks は有効"
    );

    fs::remove_dir_all(&dir).unwrap();
}

/// 未知のキーは無視され、読み込みは成功すること。
#[test]
fn unknown_keys_are_ignored() {
    let dir = test_dir("unknown");
    let user = dir.join("user.toml");
    write(
        &user,
        "[chunk]\nunload_after_secs = 123\nunknown_key = true\n[future]\nsomething = 1\n",
    );

    let settings = AppSettings::load(Some(&user), None, None).unwrap();
    assert_eq!(settings.chunk_unload_after_secs, 123);

    fs::remove_dir_all(&dir).unwrap();
}

/// 存在しないファイルは飛ばされ、壊れた TOML は見える失敗になること。
#[test]
fn missing_files_are_skipped_and_broken_toml_fails() {
    let dir = test_dir("missing");
    let absent = dir.join("no-such-file.toml");
    let settings = AppSettings::load(Some(&absent), None, None).unwrap();
    assert_eq!(settings, AppSettings::default());

    let broken = dir.join("broken.toml");
    write(&broken, "[chunk\nunload_after_secs = ");
    assert!(
        AppSettings::load(Some(&broken), None, None).is_err(),
        "壊れた TOML は黙って既定値に落ちてはいけない"
    );

    fs::remove_dir_all(&dir).unwrap();
}
