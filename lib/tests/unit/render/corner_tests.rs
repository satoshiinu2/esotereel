use std::collections::BTreeMap;

use super::*;
use crate::plugin::{
    NamespacedID,
    property::{PropertyKey, value::FieldTypeKind},
};

fn corner_field(name: &str, capability: ComponentCapability, default: [f64; 2]) -> PropertySchema {
    let owner = NamespacedID::new("std", "corner_deform").unwrap();
    PropertySchema {
        key: PropertyKey::new(owner, name).unwrap(),
        category: vec![],
        label: name.to_string(),
        kind: FieldTypeKind::Array {
            item_kind: Box::new(FieldTypeKind::Float {
                min: None,
                max: None,
                step: 0.01,
            }),
        },
        default: FieldValue::Array(vec![
            FieldValue::Float(default[0]),
            FieldValue::Float(default[1]),
        ]),
        capability: Some(capability),
    }
}

/// メンバー名は能力解決を検証するため、隅の名前とは無関係なものを使う。
fn corner_schema() -> PropertySchema {
    let owner = NamespacedID::new("std", "corner_deform").unwrap();
    let fields = vec![
        corner_field("a", ComponentCapability::CornerTopLeft, [0.0, 0.0]),
        corner_field("b", ComponentCapability::CornerTopRight, [0.0, 0.0]),
        corner_field("c", ComponentCapability::CornerBottomRight, [1.0, 2.0]),
        corner_field("d", ComponentCapability::CornerBottomLeft, [0.0, 0.0]),
    ];
    let mut schema = PropertySchema::group(
        PropertyKey::new(owner, "corner_deform").unwrap(),
        "Corner Deform".to_string(),
        fields,
    );
    schema.capability = Some(ComponentCapability::CornerDeformable);
    schema
}

fn offset_value(x: f64, y: f64) -> FieldValue {
    FieldValue::Array(vec![FieldValue::Float(x), FieldValue::Float(y)])
}

#[test]
fn from_property_resolves_members_by_capability() {
    let schema = corner_schema();
    let value = FieldValue::Map(BTreeMap::from([
        ("a".to_string(), offset_value(0.5, -0.25)),
        ("b".to_string(), offset_value(1.0, 0.0)),
        ("d".to_string(), offset_value(0.0, 3.0)),
    ]));
    let offsets = CornerOffsets::from_property(&schema, &value).unwrap();
    assert_eq!(offsets.top_left, [0.5, -0.25]);
    assert_eq!(offsets.top_right, [1.0, 0.0]);
    // 値が無い隅はスキーマのデフォルトを使う
    assert_eq!(offsets.bottom_right, [1.0, 2.0]);
    assert_eq!(offsets.bottom_left, [0.0, 3.0]);
}

#[test]
fn from_property_rejects_malformed_values() {
    let schema = corner_schema();
    let wrong_len = FieldValue::Map(BTreeMap::from([(
        "a".to_string(),
        FieldValue::Array(vec![FieldValue::Float(1.0)]),
    )]));
    assert!(CornerOffsets::from_property(&schema, &wrong_len).is_err());

    let non_number = FieldValue::Map(BTreeMap::from([(
        "a".to_string(),
        FieldValue::Array(vec![
            FieldValue::String("x".to_string()),
            FieldValue::Float(0.0),
        ]),
    )]));
    assert!(CornerOffsets::from_property(&schema, &non_number).is_err());

    assert!(CornerOffsets::from_property(&schema, &FieldValue::Float(1.0)).is_err());
}

#[test]
fn from_property_rejects_missing_member_capability() {
    let owner = NamespacedID::new("std", "corner_deform").unwrap();
    let fields = vec![corner_field(
        "a",
        ComponentCapability::CornerTopLeft,
        [0.0, 0.0],
    )];
    let schema = PropertySchema::group(
        PropertyKey::new(owner, "corner_deform").unwrap(),
        "Corner Deform".to_string(),
        fields,
    );
    let value = FieldValue::Map(BTreeMap::new());
    assert!(CornerOffsets::from_property(&schema, &value).is_err());
}

#[test]
fn stack_entry_carries_offsets_as_shader_params() {
    let offsets = CornerOffsets {
        top_left: [0.5, -0.25],
        top_right: [1.0, 0.0],
        bottom_right: [-0.5, 2.0],
        bottom_left: [0.0, 3.0],
    };
    let id = corner_deform_shader_id().unwrap();
    let entry = offsets.stack_entry(id.clone());
    assert_eq!(entry.shader_id, id);
    // キーは std:corner_deform シェーダが宣言する固有プロパティのローカル名
    let expected = [
        ("top_left_x", 0.5),
        ("top_left_y", -0.25),
        ("top_right_x", 1.0),
        ("top_right_y", 0.0),
        ("bottom_right_x", -0.5),
        ("bottom_right_y", 2.0),
        ("bottom_left_x", 0.0),
        ("bottom_left_y", 3.0),
    ];
    assert_eq!(entry.properties.len(), expected.len());
    for (key, value) in expected {
        assert_eq!(
            entry.properties.get(key),
            Some(&FieldValue::Float(value)),
            "key {key}"
        );
    }
}

#[test]
fn identity_offsets_are_detected() {
    assert!(CornerOffsets::default().is_identity());
    assert!(!CornerOffsets {
        top_left: [0.1, 0.0],
        ..Default::default()
    }
    .is_identity());
}
