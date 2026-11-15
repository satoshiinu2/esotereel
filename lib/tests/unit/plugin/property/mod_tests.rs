use super::*;

#[test]
fn parse_one_string_with_default() {
    let raw = PropertySchemaRaw {
        key: "source".to_string(),
        label: "Source".to_string(),
        category: vec!["Media".to_string()],
        kind: toml::Value::Table(
            [(
                "type".to_string(),
                toml::Value::String("string".to_string()),
            )]
            .into_iter()
            .collect(),
        ),
        default: Some(toml::Value::String("default.mp4".to_string())),
        capability: None,
        member_capabilities: BTreeMap::new(),
        fields: None,
    };
    let schema = PropertySchema::parse_one(raw, "std").unwrap();
    assert_eq!(schema.key.full(), "std:source");
    assert_eq!(schema.label, "Source");
    assert_eq!(schema.category, vec!["Media"]);
    assert!(matches!(schema.kind, FieldTypeKind::String));
    assert_eq!(
        schema.default,
        FieldValue::String("default.mp4".to_string())
    );
}

#[test]
fn parse_one_derives_default_when_omitted() {
    let raw = PropertySchemaRaw {
        key: "count".to_string(),
        label: "Count".to_string(),
        category: vec![],
        kind: toml::Value::Table(
            [("type".to_string(), toml::Value::String("int".to_string()))]
                .into_iter()
                .collect(),
        ),
        default: None,
        capability: None,
        member_capabilities: BTreeMap::new(),
        fields: None,
    };
    let schema = PropertySchema::parse_one(raw, "std").unwrap();
    assert_eq!(schema.default, FieldValue::Int(0));
}

#[test]
fn parse_one_rejects_unknown_kind() {
    let raw = PropertySchemaRaw {
        key: "bad".to_string(),
        label: "Bad".to_string(),
        category: vec![],
        kind: toml::Value::String("not-a-kind".to_string()),
        default: None,
        capability: None,
        member_capabilities: BTreeMap::new(),
        fields: None,
    };
    assert!(PropertySchema::parse_one(raw, "std").is_err());
}

#[test]
fn validate_properties_rejects_duplicate_keys() {
    let key = NamespacedID::new("std", "dup").unwrap();
    let props = vec![
        PropertySchema {
            key: key.clone(),
            category: vec![],
            label: "First".to_string(),
            kind: FieldTypeKind::String,
            default: FieldValue::String("a".to_string()),
            capability: None,
            member_capabilities: BTreeMap::new(),
        },
        PropertySchema {
            key,
            category: vec![],
            label: "Second".to_string(),
            kind: FieldTypeKind::String,
            default: FieldValue::String("b".to_string()),
            capability: None,
            member_capabilities: BTreeMap::new(),
        },
    ];
    assert!(PropertySchema::validate_properties(&props).is_err());
}

#[test]
fn group_constructor_builds_nested_default() {
    let child = PropertySchema {
        key: NamespacedID::new("std", "font_size").unwrap(),
        category: vec![],
        label: "Font Size".to_string(),
        kind: FieldTypeKind::Float {
            min: None,
            max: None,
            step: 1.0,
        },
        default: FieldValue::Float(24.0),
        capability: None,
        member_capabilities: BTreeMap::new(),
    };
    let group = PropertySchema::group(
        NamespacedID::new("std", "style").unwrap(),
        "Style".to_string(),
        vec![child],
    );
    // グループはフィールド宣言だけで構成され、デフォルト値は
    // 子フィールドのデフォルトを集めた Map になる。
    assert!(matches!(
        group.kind,
        FieldTypeKind::Group { ref fields, .. }
            if fields.len() == 1 && fields[0].key.full() == "std:font_size"
    ));
    assert!(matches!(
        group.default,
        FieldValue::Map(ref m) if m.get("font_size") == Some(&FieldValue::Float(24.0))
    ));
}

#[test]
fn parse_toml_rejects_duplicate_property_keys() {
    let text = r#"
[[fields]]
key = "dup"
label = "Dup"
kind = { type = "string" }
[[fields]]
key = "dup"
label = "Dup again"
kind = { type = "string" }
"#;
    assert!(PropertySchema::parse_toml(text, "std").is_err());
}

#[test]
fn leaf_fields_flattens_group_members() {
    let inner = PropertySchema {
        key: NamespacedID::new("std", "x").unwrap(),
        category: vec![],
        label: "X".to_string(),
        kind: FieldTypeKind::Int {
            min: None,
            max: None,
        },
        default: FieldValue::Int(1),
        capability: None,
        member_capabilities: BTreeMap::new(),
    };
    let group = PropertySchema::group(
        NamespacedID::new("std", "g").unwrap(),
        "Group".to_string(),
        vec![inner],
    );
    let leaves = PropertySchema::leaf_fields(&[group]).unwrap();
    assert_eq!(leaves.len(), 1);
    assert_eq!(leaves[0].0.to_string(), "std:g/x");
    assert_eq!(leaves[0].1.label, "X");
}
