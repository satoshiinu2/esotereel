use anyhow::{Context, Result};

use crate::plugin::toolbar::ToolbarAction;

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

        let action = ToolbarAction {
            func_name: get_string(table, "entry")?,
        };

        Ok(action)
    }
}

fn get_string(table: &toml::value::Table, key: &str) -> Result<String> {
    table
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .with_context(|| format!("missing or invalid string field `{key}`"))
}
