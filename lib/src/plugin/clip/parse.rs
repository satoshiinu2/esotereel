use std::collections::{HashMap, HashSet};

use anyhow::Context;

use crate::{
    plugin::{
        NamespacedID,
        property::{PropertySchema, parse::PropertySchemaRaw},
    },
    project::clip::ClipKind,
};

#[derive(Debug, serde::Deserialize)]
struct ClipKindRaw {
    id: String,
    render_script: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PropertyRaw {
    target: Vec<String>,
    #[serde(default)]
    fields: Vec<PropertySchemaRaw>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct ClipKindFile {
    #[serde(default)]
    kinds: Vec<ClipKindRaw>,
    #[serde(default)]
    properties: Vec<PropertyRaw>,
}

/// プロパティの適用先を表す。
///
/// - `Kind`: `plugin_id:local_id` 形式のフル修飾ID
/// - `Tag`: `#tag_name` 形式。そのタグを持つ全クリップに適用
/// - `All`: `*`。全クリップに適用(他のtargetと混在していたら無視して全適用)
#[derive(Debug, Clone)]
pub enum Target {
    Kind(NamespacedID),
    Tag(String),
    All,
}

impl Target {
    fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "*" => Ok(Self::All),
            s if s.starts_with('#') => {
                let tag = &s[1..];
                if tag.is_empty() {
                    anyhow::bail!("empty tag in target `{}`", s);
                }
                Ok(Self::Tag(tag.to_string()))
            }
            s => Ok(Self::Kind(NamespacedID::parse(s)?)),
        }
    }
}

/// まだどのClipKindにも適用していない、target+fieldsの組。
/// 全プラグインのkindsが出揃うまで解決を遅延させるための中間表現。
#[derive(Debug, Clone)]
pub struct PendingProperty {
    pub targets: Vec<Target>,
    pub fields: Vec<PropertySchema>,
}

impl ClipKind {
    /// kindsだけを確定させて返す。propertiesはpendingのまま返し、まだ適用しない。
    /// 適用は他プラグイン分も含めて全kindsが出揃った後、`resolve_and_apply` で行う。
    pub fn parse_toml(
        text: &str,
        plugin_id: &str,
    ) -> anyhow::Result<(HashMap<NamespacedID, ClipKind>, Vec<PendingProperty>)> {
        let parsed: ClipKindFile = toml::from_str(text).map_err(|e| {
            anyhow::anyhow!("failed to parse clip kinds TOML in {}: {}", plugin_id, e)
        })?;

        let kinds = Self::parse_kinds(parsed.kinds, plugin_id)?;

        let pending = parsed
            .properties
            .into_iter()
            .map(|p| Self::parse_pending(p, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok((kinds, pending))
    }

    fn parse_kinds(
        raw_kinds: Vec<ClipKindRaw>,
        plugin_id: &str,
    ) -> anyhow::Result<HashMap<NamespacedID, ClipKind>> {
        let mut kinds = HashMap::new();
        for raw in raw_kinds {
            let id = NamespacedID::new(plugin_id, &raw.id)?;
            let kind = ClipKind {
                render_script: raw.render_script,
                tags: raw.tags.into_iter().collect(),
            };
            if kinds.insert(id.clone(), kind).is_some() {
                anyhow::bail!("duplicate clip kind id `{}`", id);
            }
        }
        Ok(kinds)
    }

    fn parse_pending(prop: PropertyRaw, plugin_id: &str) -> anyhow::Result<PendingProperty> {
        let targets = prop
            .target
            .iter()
            .map(|s| Target::parse(s))
            .collect::<anyhow::Result<Vec<_>>>()
            .with_context(|| format!("invalid property target in `{plugin_id}`"))?;

        if targets.is_empty() {
            anyhow::bail!("property in `{plugin_id}` has no target");
        }

        let fields = prop
            .fields
            .into_iter()
            .map(|f| PropertySchema::parse_one(f, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(PendingProperty { targets, fields })
    }

    pub fn resolve_properties(
        kinds: &HashMap<NamespacedID, ClipKind>,
        pending: Vec<PendingProperty>,
    ) -> anyhow::Result<HashMap<NamespacedID, Vec<PropertySchema>>> {
        let mut index: HashMap<NamespacedID, Vec<PropertySchema>> = HashMap::new();

        for p in pending {
            let matched: Vec<NamespacedID> = if p.targets.iter().any(|t| matches!(t, Target::All)) {
                kinds.keys().cloned().collect()
            } else {
                let mut matched = HashSet::new();
                for target in &p.targets {
                    match target {
                        Target::Kind(id) => {
                            if !kinds.contains_key(id) {
                                anyhow::bail!("property target clip kind `{}` not found", id);
                            }
                            matched.insert(id.clone());
                        }
                        Target::Tag(tag) => matched.extend(
                            kinds
                                .iter()
                                .filter(|(_, k)| k.tags.contains(tag))
                                .map(|(id, _)| id.clone()),
                        ),
                        Target::All => unreachable!(),
                    }
                }
                matched.into_iter().collect()
            };

            for id in matched {
                index.entry(id).or_default().extend(p.fields.clone());
            }
        }

        for fields in index.values() {
            PropertySchema::validate_properties(fields)?;
        }

        Ok(index)
    }
}
