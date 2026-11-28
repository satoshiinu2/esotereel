use super::{PluginManifest, PluginPermission};

#[test]
fn manifest_parses_property_read_permission() {
    let manifest: PluginManifest = toml::from_str(
        r#"
id = "reader"
name = "Reader"
version = "0.1.0"
permissions = ["property_read"]
"#,
    )
    .unwrap();
    assert!(manifest.allows(PluginPermission::PropertyRead));
    assert!(!manifest.allows(PluginPermission::PropertyWrite));
}

#[test]
fn manifest_rejects_unknown_permission() {
    let result: Result<PluginManifest, _> = toml::from_str(
        r#"
id = "reader"
name = "Reader"
version = "0.1.0"
permissions = ["bogus_permission"]
"#,
    );
    assert!(result.is_err());
}
