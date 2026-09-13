use std::collections::HashSet;

use anyhow::Context;

use crate::plugin::{
    NamespacedID,
    property::{PropertySchema, parse::PropertySchemaRaw},
};

#[derive(Debug, serde::Deserialize)]
struct ClipKindRaw {
    id: String,
    func_name: String,
    #[serde(default)]
    fields: Vec<PropertySchemaRaw>,
}

#[derive(Debug, serde::Deserialize)]
struct ClipKindFile {
    kinds: Vec<ClipKindRaw>,
}

#[derive(Debug, Clone)]
pub struct ClipKind {
    pub id: NamespacedID,
    pub func_name: String,
    pub property_schema: Vec<PropertySchema>,
}

impl ClipKind {
    pub fn parse_toml(text: &str, plugin_id: &str) -> anyhow::Result<Vec<ClipKind>> {
        let parsed: ClipKindFile = toml::from_str(text).map_err(|e| {
            anyhow::anyhow!("failed to parse clip kinds TOML in {}: {}", plugin_id, e)
        })?;

        let kinds = parsed
            .kinds
            .into_iter()
            .map(|raw| Self::parse_one(raw, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Self::validate_kinds(&kinds)
            .with_context(|| format!("clip kind validation failed in `{plugin_id}`"))?;

        Ok(kinds)
    }

    fn parse_one(raw: ClipKindRaw, plugin_id: &str) -> anyhow::Result<ClipKind> {
        let id = NamespacedID::new(plugin_id, &raw.id)?;

        let property_schema = raw
            .fields
            .into_iter()
            .map(|f| PropertySchema::parse_one(f, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        // このClipKind内だけで閉じたキー空間として検証する
        // (他のClipKindや設定のキーとは無関係)
        PropertySchema::validate_fields(&property_schema)
            .with_context(|| format!("invalid property schema for clip kind `{id}`"))?;

        Ok(ClipKind {
            id,
            func_name: raw.func_name,
            property_schema,
        })
    }

    pub fn validate_kinds(kinds: &[ClipKind]) -> anyhow::Result<()> {
        let mut seen = HashSet::new();
        for kind in kinds {
            if !seen.insert(&kind.id) {
                anyhow::bail!("duplicate clip kind id `{}`", kind.id);
            }
        }
        Ok(())
    }
}
