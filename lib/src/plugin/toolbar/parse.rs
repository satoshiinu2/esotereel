use anyhow::{Context, Result};

use crate::plugin::toolbar::{ToolbarAction, ToolbarRunOn};

#[derive(Debug, serde::Deserialize)]
pub(super) struct ToolbarButtonRaw {
    pub(super) id: String,
    #[serde(default)]
    pub(super) target: Option<String>,
    pub(super) label: String,
    #[serde(default)]
    pub(super) tooltip: Option<String>,
    #[serde(default)]
    pub(super) icon: Option<String>,
    pub(super) action: toml::Value,
}

impl ToolbarAction {
    pub(super) fn from_toml_value(v: &toml::Value) -> Result<Self> {
        let table = v
            .as_table()
            .context("`action` must be a table (e.g. `action = { type = \"builtin\", command = \"zoom_in\" }`)")?;

        let func_name = get_string(table, "entry")?;

        let run_on = if let Some(run_on_str) = get_optional_string(table, "run_on")? {
            match run_on_str.to_lowercase().as_str() {
                "client" => ToolbarRunOn::Client,
                "server" => ToolbarRunOn::Server,
                _ => return Err(anyhow::anyhow!("invalid run_on value: must be 'client' or 'server'")),
            }
        } else {
            ToolbarRunOn::Client // default
        };

        let action = ToolbarAction {
            func_name,
            run_on,
        };

        Ok(action)
    }
}

fn get_optional_string(table: &toml::value::Table, key: &str) -> Result<Option<String>> {
    Ok(table.get(key).and_then(|v| v.as_str()).map(|s| s.to_string()))
}

fn get_string(table: &toml::value::Table, key: &str) -> Result<String> {
    table
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .with_context(|| format!("missing or invalid string field `{key}`"))
}
