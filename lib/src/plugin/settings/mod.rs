use std::{borrow::Cow, collections::HashMap, fs, path::Path};

use anyhow::Context;

use crate::plugin::{
    NamespacedID,
    property::{PropertyHost, PropertySchema, value::FieldValue},
};

#[derive(Debug, Default)]
pub struct SettingsSchemaRegistry {
    fields: Vec<PropertySchema>,
}

impl SettingsSchemaRegistry {
    pub fn fields(&self) -> &[PropertySchema] {
        &self.fields
    }

    pub fn from_fields(fields: Vec<PropertySchema>) -> Self {
        Self { fields }
    }
}

#[derive(Debug, Default)]
pub struct SettingsStore {
    pub schema: SettingsSchemaRegistry,
    values: HashMap<NamespacedID, FieldValue>,
    pending_values: HashMap<String, toml::Value>,
}

impl SettingsStore {
    pub fn from_fields(fields: Vec<PropertySchema>) -> Self {
        Self {
            schema: SettingsSchemaRegistry::from_fields(fields),
            values: HashMap::new(),
            pending_values: HashMap::new(),
        }
    }

    pub fn from_plugin_fields(plugin_fields: Vec<PropertySchema>) -> Self {
        Self::from_fields(plugin_fields)
    }

    pub fn add_missing_from_schema(&mut self) {
        for f in self.schema.fields() {
            self.values
                .entry(f.key.clone())
                .or_insert_with(|| f.default.clone());
        }
        self.apply_pending_values();
    }

    /// 既存の設定スキーマと値を保ったまま、新しいスキーマを追加する。
    pub fn merge_fields(&mut self, fields: Vec<PropertySchema>) -> anyhow::Result<()> {
        for field in fields {
            if self
                .schema
                .fields
                .iter()
                .any(|existing| existing.key == field.key)
            {
                anyhow::bail!("duplicate settings key: {}", field.key);
            }
            self.values
                .entry(field.key.clone())
                .or_insert_with(|| field.default.clone());
            self.schema.fields.push(field);
        }
        self.apply_pending_values();
        Ok(())
    }

    /// 未知のキーは保留し、後からプラグインスキーマが追加された時に適用する。
    pub async fn load_from_path(&mut self, path: &Path) -> anyhow::Result<()> {
        self.pending_values = load_settings_values(path).await?;
        self.apply_pending_values();
        Ok(())
    }

    /// プラグイン読み込み後の通常設定を同期的に読み込む。
    pub fn load_from_path_sync(&mut self, path: &Path) -> anyhow::Result<()> {
        if !path.exists() {
            self.pending_values.clear();
            return Ok(());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read settings file {}", path.display()))?;
        let table: toml::Value = toml::from_str(&text)
            .with_context(|| format!("failed to parse settings file {}", path.display()))?;
        self.pending_values = table
            .as_table()
            .context("settings file root is not a table")?
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        self.apply_pending_values();
        Ok(())
    }

    fn apply_pending_values(&mut self) {
        for field in &self.schema.fields {
            if field.key.plugin_id() == "core" {
                continue;
            }
            let Some(value) = self.pending_values.get(field.key.full()) else {
                continue;
            };
            match field.kind.parse_toml_to(value) {
                Ok(value) => {
                    self.values.insert(field.key.clone(), value);
                }
                Err(error) => {
                    log::warn!("Ignoring invalid setting {}: {}", field.key, error);
                }
            }
        }
    }

    /// 現在認識している値と未知キーをまとめてTOMLへ保存する。
    pub fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        let mut table = toml::map::Map::new();
        for (key, value) in &self.pending_values {
            if key.starts_with("core:") {
                continue;
            }
            table.insert(key.clone(), value.clone());
        }
        for field in &self.schema.fields {
            if field.key.plugin_id() == "core" {
                continue;
            }
            let Some(value) = self.values.get(&field.key) else {
                continue;
            };
            let value = field_value_to_toml(&field.kind, value)
                .with_context(|| format!("cannot serialize setting {}", field.key))?;
            table.insert(field.key.full().to_owned(), value);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create settings directory {}", parent.display())
            })?;
        }
        let text = toml::to_string_pretty(&toml::Value::Table(table))?;
        fs::write(path, text)
            .with_context(|| format!("failed to write settings file {}", path.display()))?;
        Ok(())
    }

    /// ディスクから読み込めた値だけ上書き。存在しないキーはdefaultのまま残る。
    pub fn apply_loaded_setting(&mut self, loaded: HashMap<NamespacedID, FieldValue>) {
        for (k, v) in loaded {
            self.values.insert(k, v);
        }
    }

    pub fn get_all_fields(&self) -> Vec<PropertySchema> {
        self.schema.fields().to_vec()
    }

    pub fn get_value(&self, key: &NamespacedID) -> Option<&FieldValue> {
        self.values.get(key)
    }

    pub fn set_value(&mut self, key: NamespacedID, value: FieldValue) -> anyhow::Result<()> {
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

fn field_value_to_toml(
    kind: &crate::plugin::property::value::FieldTypeKind,
    value: &FieldValue,
) -> anyhow::Result<toml::Value> {
    use crate::plugin::property::value::FieldTypeKind;

    let value = match (kind, value) {
        (FieldTypeKind::Bool, FieldValue::Bool(value)) => toml::Value::Boolean(*value),
        (FieldTypeKind::Int { .. }, FieldValue::Int(value)) => toml::Value::Integer(*value),
        (FieldTypeKind::Float { .. }, FieldValue::Float(value)) => toml::Value::Float(*value),
        (FieldTypeKind::Enum { .. }, FieldValue::Enum(value) | FieldValue::String(value))
        | (FieldTypeKind::String, FieldValue::String(value))
        | (FieldTypeKind::FilePath { .. }, FieldValue::String(value))
        | (FieldTypeKind::Color, FieldValue::String(value)) => toml::Value::String(value.clone()),
        (FieldTypeKind::FilePath { .. }, FieldValue::Path(paths)) => toml::Value::Array(
            paths
                .iter()
                .map(|path| toml::Value::String(path.to_string_lossy().into_owned()))
                .collect(),
        ),
        (FieldTypeKind::Color, FieldValue::Color(color)) => toml::Value::String(format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
            (color.a * 255.0) as u8
        )),
        (FieldTypeKind::Array { item_kind }, FieldValue::Array(values)) => toml::Value::Array(
            values
                .iter()
                .map(|value| field_value_to_toml(item_kind, value))
                .collect::<anyhow::Result<Vec<_>>>()?,
        ),
        (FieldTypeKind::Map { value_kind, .. }, FieldValue::Map(values)) => {
            let mut map = toml::map::Map::new();
            for (key, value) in values {
                map.insert(key.clone(), field_value_to_toml(value_kind, value)?);
            }
            toml::Value::Table(map)
        }
        _ => anyhow::bail!("field value does not match its setting kind"),
    };
    Ok(value)
}

impl PropertyHost for SettingsStore {
    fn properties(&self) -> Cow<'_, [PropertySchema]> {
        Cow::Borrowed(self.schema.fields())
    }

    fn get_value(&self, key: &NamespacedID) -> Option<&FieldValue> {
        self.values.get(key)
    }

    fn set_value(&mut self, key: NamespacedID, value: FieldValue) -> anyhow::Result<()> {
        if !self.schema.fields().iter().any(|f| f.key == key) {
            anyhow::bail!("unknown settings key: {}", key);
        }
        self.values.insert(key, value);
        Ok(())
    }
}

// TODO:
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
