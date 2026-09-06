use std::{collections::HashMap, path::Path};

use anyhow::Context;
use colored::Color;

use crate::setting::parse::FieldSchemaRaw;

mod parse;

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
    Color(Color),
    Array(Vec<FieldKind>),
    Map(HashMap<String, FieldKind>),
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FieldSchema {
    pub key: String,
    pub category: Vec<String>,
    pub label: String,
    pub kind: FieldTypeKind,
    pub default: toml::Value,
}

impl FieldSchema {
    pub fn parse_toml(text: &str, source_name: &str) -> anyhow::Result<Vec<FieldSchema>> {
        let parsed: SchemaFile = toml::from_str(text).map_err(|e| {
            anyhow::anyhow!(
                "failed to parse settings schema TOML in {}: {}",
                source_name,
                e
            )
        })?;

        let fields = parsed
            .fields
            .into_iter()
            .map(|raw| Self::parse_fields(raw, source_name))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Self::validate_fields(&fields)
            .with_context(|| format!("schema validation failed in `{source_name}`"))?;

        Ok(fields)
    }

    fn parse_fields(raw: FieldSchemaRaw, source_name: &str) -> anyhow::Result<FieldSchema> {
        let kind = FieldTypeKind::from_toml_value(&raw.kind)
            .with_context(|| format!("invalid `kind` for key `{}` in `{source_name}`", raw.key))?;

        // default定義されていなかったらフォールバック
        let default = match raw.default {
            Some(v) => v,
            None => kind.to_default_value().with_context(|| {
                format!(
                    "could not derive default for key `{}` in `{source_name}`",
                    raw.key
                )
            })?,
        };

        Ok(FieldSchema {
            key: raw.key,
            category: raw.category,
            label: raw.label,
            kind,
            default,
        })
    }

    fn validate_fields(fields: &[FieldSchema]) -> anyhow::Result<()> {
        let mut seen = std::collections::HashSet::new();
        for field in fields {
            if !seen.insert(field.key.as_str()) {
                anyhow::bail!("duplicate settings key `{}`", field.key,);
            }
            Self::validate_kind(&field.key, &field.kind)?;
        }
        Ok(())
    }

    fn validate_kind(key: &str, kind: &FieldTypeKind) -> anyhow::Result<()> {
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

#[derive(Debug, serde::Deserialize)]
struct SchemaFile {
    fields: Vec<FieldSchemaRaw>,
}

#[derive(Debug, Default)]
pub struct SchemaRegistry {
    fields: Vec<FieldSchema>,
}

impl SchemaRegistry {
    pub fn register(&mut self, field: FieldSchema) {
        self.fields.push(field);
    }

    pub fn fields(&self) -> &[FieldSchema] {
        &self.fields
    }

    /// プラグイン由来のスキーマ(namespace済み)を合流させる。
    /// 組み込み/プラグイン間・プラグイン同士でキー衝突があればエラーにする。
    pub fn merge_plugin_fields(&mut self, plugin_fields: Vec<FieldSchema>) -> anyhow::Result<()> {
        let mut merged = self.fields.clone();
        merged.extend(plugin_fields);
        FieldSchema::validate_fields(&merged)
            .context("plugin schema conflicts with existing settings")?;
        self.fields = merged;
        Ok(())
    }
}

#[derive(Debug)]
pub struct SettingsStore {
    values: HashMap<String, toml::Value>,
    has_disk_loaded: bool,
}

impl SettingsStore {
    /// スキーマだけで初期化。ディスクはまだ読んでない。
    pub fn from_schema(schema: &SchemaRegistry) -> Self {
        let values = schema
            .fields()
            .into_iter()
            .map(|f| (f.key.clone(), f.default.clone()))
            .collect();
        Self {
            values,
            has_disk_loaded: false,
        }
    }

    /// ディスクから読み込めた値だけ上書き。存在しないキーはdefaultのまま残る。
    pub fn apply_loaded(&mut self, loaded: HashMap<String, toml::Value>) {
        for (k, v) in loaded {
            self.values.insert(k, v);
        }
        self.has_disk_loaded = true;
    }

    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        self.values.get(key)
    }

    pub fn set(&mut self, key: String, value: toml::Value) {
        self.values.insert(key, value);
    }

    pub fn fill_missing_defaults(&mut self, schema: &SchemaRegistry) {
        for f in schema.fields() {
            self.values
                .entry(f.key.clone())
                .or_insert_with(|| f.default.clone());
        }
    }
}

pub async fn load_settings_values(path: &Path) -> anyhow::Result<HashMap<String, toml::Value>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let text = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read settings file at {}", path.display()))?;
    let table: toml::Value = toml::from_str(&text)
        .with_context(|| format!("failed to parse settings file at {}", path.display()))?;
    let map = table
        .as_table()
        .ok_or_else(|| anyhow::anyhow!("settings file root is not a table"))?
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Ok(map)
}
