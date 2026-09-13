use std::{collections::HashMap, path::Path};

use anyhow::Context;

use crate::plugin::{NamespacedID, property::PropertySchema};

#[derive(Debug, Default)]
pub struct SettingsSchemaRegistry {
    fields: Vec<PropertySchema>,
}

impl SettingsSchemaRegistry {
    pub fn fields(&self) -> &[PropertySchema] {
        &self.fields
    }

    /// プラグイン由来のスキーマ(namespace済み)を合流させる。
    /// 組み込み/プラグイン間・プラグイン同士でキー衝突があればエラーにする。
    pub fn merge_plugin_fields(
        &mut self,
        plugin_fields: Vec<PropertySchema>,
    ) -> anyhow::Result<()> {
        let mut merged = self.fields.clone();
        merged.extend(plugin_fields);
        PropertySchema::validate_fields(&merged)
            .context("plugin schema conflicts with existing settings")?;
        self.fields = merged;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct SettingsStore {
    pub schema: SettingsSchemaRegistry,
    values: HashMap<NamespacedID, toml::Value>,
    has_disk_loaded: bool,
}

impl SettingsStore {
    pub fn add_missing_from_schema(&mut self) {
        for f in self.schema.fields() {
            self.values
                .entry(f.key.clone())
                .or_insert_with(|| f.default.clone());
        }
    }

    /// ディスクから読み込めた値だけ上書き。存在しないキーはdefaultのまま残る。
    pub fn apply_loaded_setting(&mut self, loaded: HashMap<NamespacedID, toml::Value>) {
        for (k, v) in loaded {
            self.values.insert(k, v);
        }
        self.has_disk_loaded = true;
    }

    pub fn get_all_fields(&self) -> Vec<PropertySchema> {
        self.schema.fields().to_vec()
    }

    pub fn get_value(&self, key: &NamespacedID) -> Option<&toml::Value> {
        self.values.get(key)
    }

    pub fn set_value(&mut self, key: NamespacedID, value: toml::Value) -> anyhow::Result<()> {
        // スキーマに存在するキーかチェック
        if !self.schema.fields().iter().any(|f| f.key == key) {
            anyhow::bail!("unknown settings key: {}", key);
        }
        self.values.insert(key, value);
        Ok(())
    }

    pub fn get_categories(&self) -> Vec<String> {
        let mut categories = std::collections::HashSet::new();
        for field in self.schema.fields() {
            for cat in &field.category {
                categories.insert(cat.clone());
            }
        }
        let mut cats: Vec<_> = categories.into_iter().collect();
        cats.sort();
        cats
    }

    pub fn get_fields_by_category(&self, category: &str) -> Vec<PropertySchema> {
        self.schema
            .fields()
            .iter()
            .filter(|f| f.category.contains(&category.to_string()))
            .cloned()
            .collect()
    }

    pub fn fill_missing_defaults(&mut self, schema: &SettingsSchemaRegistry) {
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
