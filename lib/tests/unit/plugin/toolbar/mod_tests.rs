use super::*;
use std::{env, fs};

use crate::plugin::{CompiledScript, PluginLoader};

#[test]
fn parse_toolbar_button_with_script() {
    let text = r#"
[[buttons]]
id = "play"
label = "Play"
action = { entry = "toolbar_play", run_on = "client" }
script = "buttons/play.rhai"
"#;
    let buttons = ToolbarButtonSpec::parse_toml(text, "std").unwrap();
    assert_eq!(buttons.len(), 1);
    assert_eq!(buttons[0].id.full(), "std:play");
    assert_eq!(buttons[0].action.func_name, "toolbar_play");
    assert_eq!(buttons[0].script.as_str(), "buttons/play.rhai");
}

#[test]
fn toolbar_button_script_is_required() {
    let text = r#"
[[buttons]]
id = "pause"
label = "Pause"
action = { entry = "toolbar_pause" }
"#;
    assert!(ToolbarButtonSpec::parse_toml(text, "std").is_err());
}

#[test]
fn toolbar_button_script_loading_via_loader() {
    let dir = env::temp_dir().join("esotereel_toolbar_script_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("buttons")).unwrap();

    fs::write(
        dir.join("manifest.toml"),
        r#"id = "toolbar-test"
name = "Toolbar Script Test"
version = "0.1.0"
"#,
    )
    .unwrap();
    fs::write(
        dir.join("buttons").join("play.rhai"),
        "fn toolbar_play() { return (); }\n",
    )
    .unwrap();
    fs::write(
        dir.join("toolbars.toml"),
        r#"[[buttons]]
id = "play"
label = "Play"
action = { entry = "toolbar_play", run_on = "client" }
script = "buttons/play.rhai"
"#,
    )
    .unwrap();

    let mut loader = PluginLoader::new();
    loader.load_additional_plugin(&dir).unwrap();
    assert!(
        loader
            .get_toolbar_button_script(&NamespacedID::new("toolbar-test", "play").unwrap())
            .is_some()
    );

    // エンジンから直接関数を呼び出せることも確認
    let engine = rhai::Engine::new();
    let script = loader
        .get_toolbar_button_script(&NamespacedID::new("toolbar-test", "play").unwrap())
        .unwrap();
    let result: Result<(), _> = script.call::<()>(&engine, "toolbar_play", ());
    assert!(
        result.is_ok(),
        "toolbar button script should be callable: {result:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}
