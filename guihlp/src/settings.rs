use std::collections::HashSet;

use esotereel_lib::{
    plugin::{
        NamespacedID,
        property::{PropertySchema, value::FieldTypeKind},
    },
};

use crate::ffi::{
    field_value::CFieldValue,
    stringview::FfiOwnedString,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub enum SettingsFieldType {
    Bool,
    Int,
    Float,
    Enum,
    String,
    FilePath,
    Color,
    Array,
    Map,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiPropertySchema {
    pub key: FfiOwnedString,
    pub category: FfiOwnedString,
    pub label: FfiOwnedString,
    pub kind_type: SettingsFieldType,
    pub default_value: CFieldValue,
}

impl FfiPropertySchema {
    pub fn from_schema(schema: &PropertySchema) -> Self {
        let kind_type = match &schema.kind {
            FieldTypeKind::Bool => SettingsFieldType::Bool,
            FieldTypeKind::Int { .. } => SettingsFieldType::Int,
            FieldTypeKind::Float { .. } => SettingsFieldType::Float,
            FieldTypeKind::Enum { .. } => SettingsFieldType::Enum,
            FieldTypeKind::String => SettingsFieldType::String,
            FieldTypeKind::FilePath { .. } => SettingsFieldType::FilePath,
            FieldTypeKind::Color => SettingsFieldType::Color,
            FieldTypeKind::Array { .. } => SettingsFieldType::Array,
            FieldTypeKind::Map { .. } => SettingsFieldType::Map,
        };

        let category_str = schema.category.join(" > ");
        let default_value = CFieldValue::wrap_ffi(&schema.default);

        Self {
            key: FfiOwnedString::from_string(schema.key.full().to_owned()),
            category: FfiOwnedString::from_string(category_str),
            label: FfiOwnedString::from_string(schema.label.clone()),
            kind_type,
            default_value,
        }
    }
}

pub fn get_all_fields(
    settings: &esotereel_lib::plugin::settings::SettingsStore,
) -> Vec<FfiPropertySchema> {
    settings
        .schema
        .fields()
        .iter()
        .map(FfiPropertySchema::from_schema)
        .collect()
}

pub fn get_value(
    settings: &esotereel_lib::plugin::settings::SettingsStore,
    key: &str,
) -> anyhow::Result<Option<CFieldValue>> {
    let key = NamespacedID::parse(key)?;
    let value = settings
        .get_value(&key)
        .map(|value| CFieldValue::wrap_ffi(value));
    Ok(value)
}

pub fn set_value(
    settings: &mut esotereel_lib::plugin::settings::SettingsStore,
    key: &str,
    value: CFieldValue,
) -> anyhow::Result<()> {
    let key = NamespacedID::parse(key)?;
    let field_value = value.unwrap_ffi()?;
    settings.set_value(key, field_value)?;
    Ok(())
}

pub fn get_categories(
    settings: &esotereel_lib::plugin::settings::SettingsStore,
) -> Vec<String> {
    let mut categories = HashSet::new();
    for field in settings.schema.fields() {
        for cat in &field.category {
            categories.insert(cat.clone());
        }
    }
    let mut cats: Vec<_> = categories.into_iter().collect();
    cats.sort();
    cats
}
