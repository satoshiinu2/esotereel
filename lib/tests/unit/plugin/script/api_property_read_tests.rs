use super::*;

use crate::{
    plugin::{NamespacedID, property::PropertyKey},
    project::ids::IdGenerator,
};

struct StubNetwork;

impl NetworkHandler for StubNetwork {
    type Packet = Request;
    type State = ();
}

/// プロパティを2つ持つクリップ（自プラグイン所有と他プラグイン所有が1つずつ）
/// を置いたプロジェクトと、それを実行するプラグイン `reader` のコンテキストを作る。
fn context_with_clip(allow_property_read: bool) -> (PluginActionContext<StubNetwork>, ClipId) {
    let mut properties = std::collections::HashMap::new();
    properties.insert(
        PropertyKey::new(NamespacedID::new("reader", "comp").unwrap(), "volume").unwrap(),
        ClipBindingValue::Static(FieldValue::Int(7)),
    );
    properties.insert(
        PropertyKey::new(NamespacedID::new("other", "comp").unwrap(), "volume").unwrap(),
        ClipBindingValue::Static(FieldValue::Int(9)),
    );

    let mut project = Project::new();
    let timeline_id = project.insert_timeline(60.0);
    let mut ids = IdGenerator::default();
    let timeline = project.timeline_mut(timeline_id).unwrap();
    let layer = timeline
        .insert_layer(&mut ids, "Video".into(), None, None)
        .unwrap();
    let clip_id = timeline
        .new_clip_in(
            layer,
            &mut ids,
            0,
            60,
            NamespacedID::new("reader", "video").unwrap(),
            properties,
        )
        .unwrap();

    let context = PluginActionContext::new(
        Arc::new(StubNetwork),
        Arc::new(RwLock::new(Some(project))),
        timeline_id,
        "reader".to_owned(),
        false,
        allow_property_read,
    );
    (context, clip_id)
}

#[test]
fn get_property_reads_own_plugin_property_without_property_read() {
    let (context, clip_id) = context_with_clip(false);
    let value = context
        .get_clip_property(clip_id, "reader:comp#volume")
        .unwrap();
    assert_eq!(value.cast::<i64>(), 7);
}

#[test]
fn get_property_denies_other_plugin_property_without_property_read() {
    let (context, clip_id) = context_with_clip(false);
    let error = context
        .get_clip_property(clip_id, "other:comp#volume")
        .unwrap_err();
    assert!(
        error.to_string().contains("property_read"),
        "denial must name the missing permission: {error}"
    );
}

#[test]
fn get_property_reads_other_plugin_property_with_property_read() {
    let (context, clip_id) = context_with_clip(true);
    let value = context
        .get_clip_property(clip_id, "other:comp#volume")
        .unwrap();
    assert_eq!(value.cast::<i64>(), 9);
}

#[test]
fn get_property_returns_unit_for_missing_value() {
    let (context, clip_id) = context_with_clip(false);
    let value = context
        .get_clip_property(clip_id, "reader:comp#missing")
        .unwrap();
    assert!(value.is_unit());
}
