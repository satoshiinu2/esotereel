use anyhow::{Context, Result, bail};

use crate::setting::FieldTypeKind;

#[derive(Debug, serde::Deserialize)]
pub(super) struct FieldSchemaRaw {
    pub(super) key: String,
    #[serde(default)]
    pub(super) category: Vec<String>,
    pub(super) label: String,
    pub(super) kind: toml::Value,
    #[serde(default)]
    pub(super) default: Option<toml::Value>,
}

impl FieldTypeKind {
    pub(super) fn from_toml_value(v: &toml::Value) -> Result<Self> {
        let table = v
            .as_table()
            .context("`kind` must be a table (e.g. `kind = { type = \"Bool\" }`)")?;

        let ty = table
            .get("type")
            .and_then(|t| t.as_str())
            .context("`kind` table is missing a string `type` field")?;

        let kind = match ty {
            "bool" => FieldTypeKind::Bool,
            "string" => FieldTypeKind::String,
            "color" => FieldTypeKind::Color,

            "int" => FieldTypeKind::Int {
                min: get_i64(table, "min")?,
                max: get_i64(table, "max")?,
            },

            "float" => FieldTypeKind::Float {
                min: get_f64(table, "min")?,
                max: get_f64(table, "max")?,
                step: get_f64(table, "step")?,
            },

            "enum" => {
                let options = table
                    .get("options")
                    .context("Enum `kind` is missing `options`")?
                    .clone()
                    .try_into::<Vec<String>>()
                    .context("Enum `options` must be an array of strings")?;
                FieldTypeKind::Enum { options }
            }

            "array" => {
                let item_kind_val = table
                    .get("item_kind")
                    .context("Array `kind` is missing `item_kind`")?;
                let item_kind = FieldTypeKind::from_toml_value(item_kind_val)
                    .context("invalid `item_kind` in Array")?;
                FieldTypeKind::Array {
                    item_kind: Box::new(item_kind),
                }
            }

            "map" => {
                let value_kind_val = table
                    .get("value_kind")
                    .context("Map `kind` is missing `value_kind`")?;
                let value_kind = FieldTypeKind::from_toml_value(value_kind_val)
                    .context("invalid `value_kind` in Map")?;

                let known_keys = match table.get("known_keys") {
                    Some(v) => Some(
                        v.clone()
                            .try_into::<Vec<String>>()
                            .context("Map `known_keys` must be an array of strings")?,
                    ),
                    None => None,
                };

                FieldTypeKind::Map {
                    value_kind: Box::new(value_kind),
                    known_keys,
                }
            }

            other => bail!("unknown settings field kind `{other}`"),
        };

        Ok(kind)
    }

    pub fn to_default_value(&self) -> anyhow::Result<toml::Value> {
        let value = match self {
            FieldTypeKind::Bool => toml::Value::Boolean(false),

            FieldTypeKind::Int { min, max } => {
                // 0がレンジ内ならそれを、範囲外ならminを使う
                let v = if *min <= 0 && 0 <= *max { 0 } else { *min };
                toml::Value::Integer(v)
            }

            FieldTypeKind::Float { min, max, .. } => {
                let v = if *min <= 0.0 && 0.0 <= *max {
                    0.0
                } else {
                    *min
                };
                toml::Value::Float(v)
            }

            FieldTypeKind::Enum { options } => {
                let first = options
                    .first()
                    .context("Enum kind has no options to derive a default from")?;
                toml::Value::String(first.clone())
            }

            FieldTypeKind::String => toml::Value::String(String::new()),

            // Colorは表現形式次第だけど、ひとまず16進文字列運用の想定
            FieldTypeKind::Color => toml::Value::String("#FFFFFF".into()),

            FieldTypeKind::Array { .. } => toml::Value::Array(Vec::new()),

            FieldTypeKind::Map { .. } => toml::Value::Table(Default::default()),
        };
        Ok(value)
    }
}

fn get_i64(table: &toml::value::Table, key: &str) -> Result<i64> {
    table
        .get(key)
        .and_then(|v| v.as_integer())
        .with_context(|| format!("missing or invalid integer field `{key}`"))
}

fn get_f64(table: &toml::value::Table, key: &str) -> Result<f64> {
    table
        .get(key)
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .with_context(|| format!("missing or invalid float field `{key}`"))
}
