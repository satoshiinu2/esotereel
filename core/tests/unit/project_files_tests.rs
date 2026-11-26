use super::*;

use std::collections::BTreeMap;

use esotereel_lib::{
    plugin::{
        NamespacedID, PluginLoader,
        property::{PropertyKey, value::FieldValue},
    },
    project::{clip::ClipBindingValue, ids::IdGenerator},
};

#[test]
fn convert_legacy_media_sources_rewrites_string_sources_via_plugin_definitions() {
    // 旧形式プロジェクトを開くときの変換ドライバの契約:
    // クリップ種別の能力宣言から source のパスを解決し、文字列 source を
    // 登録したうえで AssetRef 値へ変換する。変換は冪等。
    let mut loader = PluginLoader::new();
    loader
        .load_additional_plugin(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plugins/std"
        )))
        .unwrap();

    let kind_id = NamespacedID::new("std", "video").unwrap();
    let media_key = PropertyKey::new(NamespacedID::new("std", "media").unwrap(), "media").unwrap();
    let properties = HashMap::from([(
        media_key,
        ClipBindingValue::Static(FieldValue::Map(BTreeMap::from([(
            "source".to_string(),
            FieldValue::String("/tmp/legacy-driver.mp4".to_string()),
        )]))),
    )]);

    let mut project = Project::new();
    let timeline_id = project.insert_timeline(60.0);
    let layer_id = *project
        .timeline_ref(timeline_id)
        .unwrap()
        .iter_layers()
        .next()
        .unwrap()
        .0;
    let mut ids = IdGenerator::default();
    let clip_id = project
        .timeline_mut(timeline_id)
        .unwrap()
        .new_clip_in(layer_id, &mut ids, 0, 50, kind_id, properties)
        .unwrap();

    assert_eq!(convert_legacy_media_sources(&mut project, &loader), 1);

    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    let source_path = PropertyPath::parse("std:media#media/source").unwrap();
    let FieldValue::AssetRef(asset_ref) = clip.get_property_path(&source_path).unwrap() else {
        panic!("legacy string source must be converted to an asset reference value");
    };
    assert_eq!(
        project.assets().resolve(*asset_ref).unwrap(),
        std::path::Path::new("/tmp/legacy-driver.mp4")
    );

    // 冪等: 変換済みのプロジェクトでは変換対象が無い。
    assert_eq!(convert_legacy_media_sources(&mut project, &loader), 0);
}
