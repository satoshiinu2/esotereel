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
            FieldTypeKind::Group { fields, .. } => {
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
