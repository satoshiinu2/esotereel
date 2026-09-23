use std::collections::{HashMap, HashSet};

use anyhow::Context;

use crate::{
    plugin::{
        NamespacedID,
        property::{PropertySchema, parse::PropertySchemaRaw},
        registry::PluginDefinitionRegistry,
    },
    project::clip::ClipKind,
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

impl ClipKind {
    pub fn parse_toml(
        text: &str,
        plugin_id: &str,
    ) -> anyhow::Result<HashMap<NamespacedID, ClipKind>> {
        let parsed: ClipKindFile = toml::from_str(text).map_err(|e| {
            anyhow::anyhow!("failed to parse clip kinds TOML in {}: {}", plugin_id, e)
        })?;

        let kinds = parsed
            .kinds
            .into_iter()
            .map(|raw| Self::parse_one(raw, plugin_id))
            .collect::<anyhow::Result<HashMap<_, _>>>()?;

        Self::validate_kinds(&kinds)
            .with_context(|| format!("clip kind validation failed in `{plugin_id}`"))?;

        Ok(kinds)
    }

    fn parse_one(raw: ClipKindRaw, plugin_id: &str) -> anyhow::Result<(NamespacedID, ClipKind)> {
        let id = NamespacedID::new(plugin_id, &raw.id)?;

        let property_schema = raw
            .fields
            .into_iter()
            .map(|f| PropertySchema::parse_one(f, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        // このClipKind内だけで閉じたキー空間として検証する
        // (他のClipKindや設定のキーとは無関係)
        PropertySchema::validate_properties(&property_schema)
            .with_context(|| format!("invalid property schema for clip kind `{id}`"))?;

        Ok((
            id,
            ClipKind {
                func_name: raw.func_name,
                property_schema,
            },
        ))
    }

    pub fn validate_kinds(kinds: &HashMap<NamespacedID, ClipKind>) -> anyhow::Result<()> {
        let mut seen = HashSet::new();
        for (id, _kind) in kinds {
            if !seen.insert(id) {
                anyhow::bail!("duplicate clip kind id `{}`", id);
            }
        }
        Ok(())
    }
}

/// 複数プラグインから合成した「どんなClipKindが存在しうるか」の一覧。
/// ToolbarRegistry / SchemaRegistry と対になる。
#[derive(Debug, Default)]
pub struct ClipKindRegistry {
    inner: PluginDefinitionRegistry<NamespacedID, ClipKind>,
}

impl ClipKindRegistry {
    pub fn get(&self, id: &NamespacedID) -> Option<&ClipKind> {
        self.inner.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NamespacedID, &ClipKind)> {
        self.inner.iter()
    }

    pub fn contains(&self, id: &NamespacedID) -> bool {
        self.inner.contains(id)
    }

    /// プラグイン由来のClipKindを合流させる。
    /// 既存(組み込み/他プラグイン)とid衝突があればエラーにする。
    pub fn merge_plugin_kinds(
        &mut self,
        plugin_kinds: HashMap<NamespacedID, ClipKind>,
    ) -> anyhow::Result<()> {
        self.inner.merge(plugin_kinds)
    }
}

/// ClipKindの実体を保持する層。今のところレイアウト(並び順)のような
/// ユーザー設定は持たないが、ToolbarStore と対になる位置づけとして分離しておく。
#[derive(Debug, Default)]
pub struct ClipKindStore {
    pub registry: ClipKindRegistry,
}

impl ClipKindStore {
    pub fn new() -> Self {
        Self {
            registry: ClipKindRegistry::default(),
        }
    }

    pub fn merge_plugin_kinds(
        &mut self,
        plugin_kinds: HashMap<NamespacedID, ClipKind>,
    ) -> anyhow::Result<()> {
        self.registry.merge_plugin_kinds(plugin_kinds)
    }

    pub fn get(&self, id: &NamespacedID) -> Option<&ClipKind> {
        self.registry.get(id)
    }
}
