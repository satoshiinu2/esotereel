use super::*;
use crate::plugin::property::value::FieldTypeKind;

#[test]
fn clip_position_get_set() {
    let clip: Clip = Clip::new(
        1u64,
        0,
        60,
        NamespacedID::new("std", "video").unwrap(),
        HashMap::new(),
    );
    assert_eq!(clip.position(), 0);

    let mut clip = clip;
    clip.set_position(120);
    assert_eq!(clip.position(), 120);
}

#[test]
fn clip_set_and_get_property_value() {
    let mut clip = Clip::new(
        1u64,
        0,
        60,
        NamespacedID::new("std", "video").unwrap(),
        HashMap::new(),
    );
    clip.set_property_value(
        NamespacedID::new("std", "source").unwrap(),
        ClipBindingValue::Static(FieldValue::String("clip.mp4".to_string())),
    )
    .unwrap();
    assert!(matches!(
        clip.get_property_value(&NamespacedID::new("std", "source").unwrap()),
        Some(ClipBindingValue::Static(FieldValue::String(v))) if v == "clip.mp4"
    ));
}

#[test]
fn clip_default_properties_from_schema() {
    let schema = vec![PropertySchema {
        key: NamespacedID::new("std", "volume").unwrap(),
        category: vec![],
        label: "Volume".to_string(),
        kind: FieldTypeKind::Float {
            min: Some(0.0),
            max: Some(1.0),
            step: 0.1,
        },
        default: FieldValue::Float(0.5),
        capability: None,
        member_capabilities: BTreeMap::new(),
    }];
    let defaults = ClipBindingValue::default_properties(&schema);
    assert_eq!(defaults.len(), 1);
    assert!(matches!(
        defaults.get(&NamespacedID::new("std", "volume").unwrap()),
        Some(ClipBindingValue::Static(FieldValue::Float(v))) if *v == 0.5
    ));
}

#[test]
fn clip_shader_stack_serde_roundtrip() {
    let mut clip = Clip::new(
        1u64,
        0,
        60,
        NamespacedID::new("std", "video").unwrap(),
        HashMap::new(),
    );
    clip.set_shader_stack(vec![
        ShaderStackEntry {
            shader_id: NamespacedID::new("std", "wave_compute").unwrap(),
            properties: HashMap::from([("amplitude".to_string(), FieldValue::Float(20.0))]),
        },
        ShaderStackEntry {
            shader_id: NamespacedID::new("std", "twist_compute").unwrap(),
            properties: HashMap::new(),
        },
    ]);

    let json = serde_json::to_string(&clip).unwrap();
    let restored: Clip = serde_json::from_str(&json).unwrap();

    // スタックの順序とプロパティ値が保存・復元で保持される。
    assert_eq!(restored.shader_stack(), clip.shader_stack());
    assert_eq!(
        restored.shader_stack()[0].shader_id,
        NamespacedID::new("std", "wave_compute").unwrap()
    );
}

#[test]
fn clip_serde_without_shader_stack_defaults_to_empty() {
    // `shader_stack` フィールドが無い旧形式の JSON も読み込める。
    let clip = Clip::new(
        1u64,
        0,
        60,
        NamespacedID::new("std", "video").unwrap(),
        HashMap::new(),
    );
    let mut value = serde_json::to_value(&clip).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("shader_stack")
        .expect("serialized clip must contain shader_stack");

    let restored: Clip = serde_json::from_value(value).unwrap();
    assert!(restored.shader_stack().is_empty());
}

#[test]
fn clip_get_set_property_path() {
    let mut clip = Clip::new(
        1u64,
        0,
        60,
        NamespacedID::new("std", "video").unwrap(),
        ClipBindingValue::default_properties(&[PropertySchema::group(
            NamespacedID::new("std", "transform").unwrap(),
            "Transform".to_string(),
            vec![PropertySchema {
                key: NamespacedID::new("std", "x").unwrap(),
                category: vec![],
                label: "X".to_string(),
                kind: FieldTypeKind::Float {
                    min: None,
                    max: None,
                    step: 1.0,
                },
                default: FieldValue::Float(10.0),
                capability: None,
                member_capabilities: BTreeMap::new(),
            }],
        )]),
    );
    let path = PropertyPath::parse("std:transform/x").unwrap();
    assert_eq!(
        clip.get_property_path(&path),
        Some(&FieldValue::Float(10.0))
    );

    clip.set_property_path(&path, ClipBindingValue::Static(FieldValue::Float(99.0)))
        .unwrap();
    assert_eq!(
        clip.get_property_path(&path),
        Some(&FieldValue::Float(99.0))
    );
}
