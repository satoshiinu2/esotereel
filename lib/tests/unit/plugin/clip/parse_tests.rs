use super::*;

const KIND: &str = r#"
[[kinds]]
id = "video"
render_script = "render"
tags = ["media"]
components = ["media", "other:transform"]
"#;

#[test]
fn parse_clip_kinds_from_toml() {
    let kinds = ClipKind::parse_toml(KIND, "std").unwrap();
    assert_eq!(kinds.len(), 1);

    let video = kinds
        .get(&NamespacedID::new("std", "video").unwrap())
        .unwrap();
    assert_eq!(video.render_script, "render");
    assert!(video.tags.contains("media"));
    // ローカル ID は自プラグインの名前空間で、`plugin:local` 形式はそのまま解決される。
    assert_eq!(
        video.components,
        vec![
            NamespacedID::new("std", "media").unwrap(),
            NamespacedID::new("other", "transform").unwrap(),
        ]
    );
}

#[test]
fn parse_clip_kinds_rejects_duplicate_ids() {
    let duplicate = format!("{KIND}\n{KIND}");
    assert!(ClipKind::parse_toml(&duplicate, "std").is_err());
}

#[test]
fn parse_component_definitions_from_component_toml() {
    let text = r#"
[[components]]
id = "video"
name = "Video"
capabilities = ["frame-provider", "prefetchable"]
editor = "video"
script = "video.rhai"
properties = [
  { key = "source", label = "Source", kind = { type = "string" } },
]
"#;
    let map = parse_component_definitions(text, "std").unwrap();
    assert_eq!(map.len(), 1);

    let video = map
        .get(&NamespacedID::new("std", "video").unwrap())
        .unwrap();
    assert!(
        video
            .capabilities
            .contains(&ComponentCapability::FrameProvider)
    );
    assert_eq!(video.properties.len(), 1);
    assert_eq!(video.properties[0].key.address(), "std:video#source");
}

#[test]
fn parse_component_definitions_rejects_duplicate_ids() {
    let text = r#"
[[components]]
id = "dup"
name = "Dup"
[[components]]
id = "dup"
name = "Dup again"
"#;
    assert!(parse_component_definitions(text, "std").is_err());
}
