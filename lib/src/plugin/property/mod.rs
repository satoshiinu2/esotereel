use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::Context;
use colored::Color;

use crate::plugin::{NamespacedID, property::parse::PropertySchemaRaw};

pub mod parse;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum FieldTypeKind {
    Bool,
    Int {
        min: i64,
        max: i64,
    },
    Float {
        min: f64,
        max: f64,
        step: f64,
    },
    Enum {
        options: Vec<String>,
    },
    String,
    FilePath {
        extensions: Option<Vec<String>>,
    },
    Color,
    Array {
        item_kind: Box<FieldTypeKind>,
    },
    Map {
        value_kind: Box<FieldTypeKind>,
        known_keys: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone)]
pub enum FieldKind {
    Bool,
    Int(i64),
    Float(f64),
    Enum(String),
    String(String),
    Path(Vec<PathBuf>),
    Color(Color),
    Array(Vec<FieldKind>),
    Map(HashMap<String, FieldKind>),
}

#[derive(Debug, serde::Deserialize)]
struct SchemaFile {
    #[serde(default)]
    fields: Vec<PropertySchemaRaw>,
}

#[derive(Debug, Clone)]
pub struct PropertySchema {
    pub key: NamespacedID,
    pub category: Vec<String>,
    pub label: String,
    pub kind: FieldTypeKind,
    pub default: toml::Value,
}

impl PropertySchema {
    pub fn parse_toml(text: &str, plugin_id: &str) -> anyhow::Result<Vec<PropertySchema>> {
        let parsed: SchemaFile = toml::from_str(text).map_err(|e| {
            anyhow::anyhow!(
                "failed to parse settings schema TOML in {}: {}",
                plugin_id,
                e
            )
        })?;

        let fields = parsed
            .fields
            .into_iter()
            .map(|raw| Self::parse_one(raw, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Self::validate_fields(&fields)
            .with_context(|| format!("schema validation failed in `{plugin_id}`"))?;

        Ok(fields)
    }

    pub fn parse_one(raw: PropertySchemaRaw, plugin_id: &str) -> anyhow::Result<PropertySchema> {
        let kind = FieldTypeKind::from_toml_value(&raw.kind)
            .with_context(|| format!("invalid `kind` for key `{}` in `{plugin_id}`", raw.key))?;

        // default定義されていなかったらフォールバック
        let default = match raw.default {
            Some(v) => v,
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

    pub fn validate_fields(fields: &[PropertySchema]) -> anyhow::Result<()> {
        let mut seen = HashSet::new();
        for field in fields {
            if !seen.insert(&field.key) {
                anyhow::bail!("duplicate settings key `{}`", field.key,);
            }
            Self::validate_kind(&field.key, &field.kind)?;
        }
        Ok(())
    }

    fn validate_kind(key: &NamespacedID, kind: &FieldTypeKind) -> anyhow::Result<()> {
        match kind {
            FieldTypeKind::Enum { options } if options.is_empty() => {
                anyhow::bail!("Enum kind for key `{key}` has no options")
            }
            FieldTypeKind::Array { item_kind } => Self::validate_kind(key, item_kind)
                .with_context(|| format!("invalid Array item_kind for key `{key}`")),
            FieldTypeKind::Map { value_kind, .. } => Self::validate_kind(key, value_kind)
                .with_context(|| format!("invalid Map value_kind for key `{key}`")),
            _ => Ok(()),
        }
    }
}
