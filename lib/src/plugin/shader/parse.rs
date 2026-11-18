use std::collections::HashSet;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::plugin::{
    NamespacedID,
    property::{PropertySchema, parse::PropertySchemaRaw},
    shader::{ShaderDefinition, ShaderKind, ShaderProperty, core_param_type},
};

#[derive(Debug, Deserialize)]
struct ShaderFileRaw {
    id: String,
    name: String,
    source: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    entry_vertex: Option<String>,
    #[serde(default)]
    entry_fragment: Option<String>,
    #[serde(default)]
    entry_compute: Option<String>,
    #[serde(default)]
    properties: Vec<ShaderPropertyRaw>,
}

#[derive(Debug, Deserialize)]
struct ShaderPropertyRaw {
    key: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    kind: Option<toml::Value>,
    #[serde(default)]
    default: Option<toml::Value>,
}

/// シェーダ定義 TOML（1 ファイル 1 定義）をパースします。
///
/// `kind` が `"compute"` の場合は `entry_compute` が必須で、それ以外（省略時を含む）は
/// render 用として `entry_vertex` / `entry_fragment` が必須です。
/// `compiled_wgsl` は空で返るため、呼び出し側（ローダー）が wesl コンパイル結果を設定します。
pub fn parse_shader_definition(text: &str, plugin_id: &str) -> anyhow::Result<ShaderDefinition> {
    let raw: ShaderFileRaw = toml::from_str(text).map_err(|e| {
        anyhow::anyhow!("failed to parse shader definition TOML in {}: {}", plugin_id, e)
    })?;

    let kind = match raw.kind.as_deref() {
        None | Some("render") => {
            if raw.entry_compute.is_some() {
                bail!("render shader `{}` must not declare `entry_compute`", raw.id);
            }
            ShaderKind::Render {
                entry_vertex: raw
                    .entry_vertex
                    .with_context(|| format!("render shader `{}` is missing `entry_vertex`", raw.id))?,
                entry_fragment: raw.entry_fragment.with_context(|| {
                    format!("render shader `{}` is missing `entry_fragment`", raw.id)
                })?,
            }
        }
        Some("compute") => {
            if raw.entry_vertex.is_some() || raw.entry_fragment.is_some() {
                bail!(
                    "compute shader `{}` must not declare `entry_vertex` / `entry_fragment`",
                    raw.id
                );
            }
            ShaderKind::Compute {
                entry_compute: raw.entry_compute.with_context(|| {
                    format!("compute shader `{}` is missing `entry_compute`", raw.id)
                })?,
            }
        }
        Some(other) => bail!("unknown shader kind `{other}` for shader `{}`", raw.id),
    };

    let mut seen = HashSet::new();
    let mut properties = Vec::with_capacity(raw.properties.len());
    for prop in raw.properties {
        if !seen.insert(prop.key.clone()) {
            bail!("duplicate property key `{}` in shader `{}`", prop.key, raw.id);
        }
        properties.push(parse_shader_property(prop, &raw.id, plugin_id)?);
    }

    Ok(ShaderDefinition {
        id: NamespacedID::new(plugin_id, &raw.id)?,
        name: raw.name,
        kind,
        source: raw.source,
        properties,
        compiled_wgsl: String::new(),
    })
}

fn parse_shader_property(
    raw: ShaderPropertyRaw,
    shader_id: &str,
    plugin_id: &str,
) -> anyhow::Result<ShaderProperty> {
    if let Some(value_type) = core_param_type(&raw.key) {
        // core: パラメータの型は lib 固定のため、kind / default の指定は誤りとして弾く。
        if raw.kind.is_some() || raw.default.is_some() {
            bail!(
                "core parameter `{}` in shader `{}` must not declare `kind` or `default` (its type is fixed by lib)",
                raw.key,
                shader_id
            );
        }
        return Ok(ShaderProperty::Core {
            key: raw.key,
            value_type,
        });
    }
    if raw.key.starts_with("core:") {
        bail!("unknown core parameter `{}` in shader `{}`", raw.key, shader_id);
    }

    // 固有プロパティはコンポーネントのプロパティスキーマと共通のパースを再利用する。
    let schema_raw = PropertySchemaRaw {
        key: raw.key.clone(),
        category: Vec::new(),
        label: raw.label.with_context(|| {
            format!("property `{}` in shader `{}` is missing `label`", raw.key, shader_id)
        })?,
        kind: raw.kind.with_context(|| {
            format!("property `{}` in shader `{}` is missing `kind`", raw.key, shader_id)
        })?,
        default: raw.default,
        capability: None,
        fields: None,
    };
    let schema = PropertySchema::parse_one(schema_raw, plugin_id)
        .with_context(|| format!("invalid property `{}` in shader `{}`", raw.key, shader_id))?;
    Ok(ShaderProperty::Own(schema))
}

#[cfg(test)]
#[path = "../../../tests/unit/plugin/shader/parse_tests.rs"]
mod tests;
