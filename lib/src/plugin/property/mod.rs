use std::{borrow::Cow, collections::HashSet};

use anyhow::Context;

use crate::plugin::{
    NamespacedID,
    property::{
        parse::PropertySchemaRaw,
        value::{FieldTypeKind, FieldValue},
    },
};

pub mod parse;
pub mod path;
pub use path::PropertyPath;
pub mod special;
pub mod value;

#[derive(Debug, serde::Deserialize)]
struct SchemaFile {
    #[serde(default)]
    fields: Vec<PropertySchemaRaw>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PropertySchema {
    pub key: NamespacedID,
    pub category: Vec<String>,
    pub label: String,
    pub kind: FieldTypeKind,
    pub default: FieldValue,
}

impl PropertySchema {
    pub fn group(
        key: NamespacedID,
        label: String,
        reference: NamespacedID,
        fields: Vec<Self>,
    ) -> Self {
        let default = FieldValue::Map(
            fields
                .iter()
                .map(|f| (f.key.local_id().to_owned(), f.default.clone()))
                .collect(),
        );
        Self {
            key,
            category: Vec::new(),
            label,
            kind: FieldTypeKind::Group { reference, fields },
            default,
        }
    }

    pub fn special(
        key: NamespacedID,
        label: String,
        reference: NamespacedID,
        fields: Vec<Self>,
    ) -> Self {
        let default = FieldValue::Map(
            fields
                .iter()
                .map(|field| (field.key.local_id().to_owned(), field.default.clone()))
                .collect(),
        );
        Self {
            key,
            category: Vec::new(),
            label,
            kind: FieldTypeKind::Special { reference, fields },
            default,
        }
    }

    /// Flatten only the editor projection. The schema and stored values remain trees.
    pub fn leaf_fields(fields: &[Self]) -> anyhow::Result<Vec<(PropertyPath, Self)>> {
        fn visit(
            field: &PropertySchema,
            path: PropertyPath,
            groups: &[String],
            default: &FieldValue,
            out: &mut Vec<(PropertyPath, PropertySchema)>,
        ) -> anyhow::Result<()> {
            if let FieldTypeKind::Group { fields, .. } = &field.kind {
                let mut groups = groups.to_vec();
                groups.push(field.label.clone());
                groups.extend(field.category.iter().cloned());
                for child in fields {
                    let effective = match default {
                        FieldValue::Map(values) => {
                            values.get(child.key.local_id()).unwrap_or(&child.default)
                        }
                        _ => &child.default,
                    };
                    visit(
                        child,
                        path.child(child.key.local_id())?,
                        &groups,
                        effective,
                        out,
                    )?;
                }
            } else {
                let mut leaf = field.clone();
                leaf.default = default.clone();
                leaf.category = groups
                    .iter()
                    .cloned()
                    .chain(field.category.iter().cloned())
                    .collect();
                out.push((path, leaf));
            }
            Ok(())
        }
        let mut out = Vec::new();
        for field in fields {
            visit(
                field,
                field.key.clone().into(),
                &[],
                &field.default,
                &mut out,
            )?;
        }
        Ok(out)
    }

    pub fn parse_toml(text: &str, plugin_id: &str) -> anyhow::Result<Vec<PropertySchema>> {
        let parsed: SchemaFile = toml::from_str(text).map_err(|e| {
            anyhow::anyhow!(
                "failed to parse settings schema TOML in {}: {}",
                plugin_id,
                e
            )
        })?;

        let properties = parsed
            .fields
            .into_iter()
            .map(|raw| Self::parse_one(raw, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Self::validate_properties(&properties)
            .with_context(|| format!("schema validation failed in `{plugin_id}`"))?;

        Ok(properties)
    }

    pub fn parse_one(raw: PropertySchemaRaw, plugin_id: &str) -> anyhow::Result<PropertySchema> {
        let kind = FieldTypeKind::from_toml_value(&raw.kind)
            .with_context(|| format!("invalid `kind` for key `{}` in `{plugin_id}`", raw.key))?;

        // default定義されていなかったらフォールバック
        let default = match raw.default {
            Some(v) => kind.parse_toml_to(&v).with_context(|| {
                format!(
                    "could not derive default for key `{}` in `{plugin_id}`",
                    raw.key
                )
            })?,
            None => kind.to_default_value().with_context(|| {
                format!(
                    "could not derive default for key `{}` in `{plugin_id}`",
                    raw.key
                )
            })?,
        };

        Ok(PropertySchema {
            key: NamespacedID::new(plugin_id, &raw.key)?,
            category: raw.category,
            label: raw.label,
            kind,
            default,
        })
    }

    pub fn validate_properties(properties: &[PropertySchema]) -> anyhow::Result<()> {
        let mut seen = HashSet::new();
        for property in properties {
            if !seen.insert(&property.key) {
                anyhow::bail!("duplicate property key `{}`", property.key,);
            }
            Self::validate_kind(&property.key, &property.kind)?;
        }
        Ok(())
    }

    fn validate_kind(key: &NamespacedID, kind: &FieldTypeKind) -> anyhow::Result<()> {
        match kind {
            FieldTypeKind::Enum { options } if options.is_empty() => {
                anyhow::bail!("Enum kind for key `{key}` has no options")
            }
            FieldTypeKind::Group { fields, .. } | FieldTypeKind::Special { fields, .. } => {
                let mut names = HashSet::new();
                for field in fields {
                    anyhow::ensure!(
                        names.insert(field.key.local_id()),
                        "duplicate group member `{}`",
                        field.key.local_id()
                    );
                }
                Self::validate_properties(fields)
            }
            FieldTypeKind::Array { item_kind } => Self::validate_kind(key, item_kind)
                .with_context(|| format!("invalid Array item_kind for key `{key}`")),
            FieldTypeKind::Map { value_kind, .. } => Self::validate_kind(key, value_kind)
                .with_context(|| format!("invalid Map value_kind for key `{key}`")),
            _ => Ok(()),
        }
    }
}

pub trait PropertyHost {
    fn properties(&self) -> Cow<'_, [PropertySchema]>;
    fn get_value(&self, key: &NamespacedID) -> Option<&FieldValue>;
    fn set_value(&mut self, key: NamespacedID, value: FieldValue) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_one_string_with_default() {
        let raw = PropertySchemaRaw {
            key: "source".to_string(),
            label: "Source".to_string(),
            category: vec!["Media".to_string()],
            kind: toml::Value::Table(
                [("type".to_string(), toml::Value::String("string".to_string()))]
                    .into_iter()
                    .collect(),
            ),
            default: Some(toml::Value::String("default.mp4".to_string())),
        };
        let schema = PropertySchema::parse_one(raw, "std").unwrap();
        assert_eq!(schema.key.full(), "std:source");
        assert_eq!(schema.label, "Source");
        assert_eq!(schema.category, vec!["Media"]);
        assert!(matches!(schema.kind, FieldTypeKind::String));
        assert_eq!(schema.default, FieldValue::String("default.mp4".to_string()));
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
            },
            PropertySchema {
                key,
                category: vec![],
                label: "Second".to_string(),
                kind: FieldTypeKind::String,
                default: FieldValue::String("b".to_string()),
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
        };
        let group = PropertySchema::group(
            NamespacedID::new("std", "style").unwrap(),
            "Style".to_string(),
            NamespacedID::new("std", "style-ref").unwrap(),
            vec![child],
        );
        assert!(matches!(
            group.kind,
            FieldTypeKind::Group { ref reference, .. } if reference.full() == "std:style-ref"
        ));
        assert!(matches!(
            group.default,
            FieldValue::Map(ref m) if m.get("font_size") == Some(&FieldValue::Float(24.0))
        ));
    }

    #[test]
    fn special_constructor_builds_special_kind() {
        let special = PropertySchema::special(
            NamespacedID::new("std", "media").unwrap(),
            "Media".to_string(),
            NamespacedID::new("core", "media").unwrap(),
            Vec::new(),
        );
        assert!(matches!(
            special.kind,
            FieldTypeKind::Special { ref reference, .. } if reference.full() == "core:media"
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
            kind: FieldTypeKind::Int { min: None, max: None },
            default: FieldValue::Int(1),
        };
        let group = PropertySchema::group(
            NamespacedID::new("std", "g").unwrap(),
            "Group".to_string(),
            NamespacedID::new("std", "g-ref").unwrap(),
            vec![inner],
        );
        let leaves = PropertySchema::leaf_fields(&[group]).unwrap();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].0.to_string(), "std:g/x");
        assert_eq!(leaves[0].1.label, "X");
    }
}
