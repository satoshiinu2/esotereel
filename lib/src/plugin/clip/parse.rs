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
    #[serde(default)]
    properties: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PropertyRaw {
    #[serde(default)]
    target: Vec<String>,
    #[serde(default)]
    name: Vec<String>,
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

/// 名前付き定義、または従来のtargetによる拡張。まだClipKindには適用しない。
/// 全プラグインのkindsが出揃うまで解決を遅延させるための中間表現。
#[derive(Debug, Clone)]
pub struct PendingProperty {
    pub names: Vec<NamespacedID>,
    pub targets: Vec<Target>,
    pub fields: Vec<PropertySchema>,
}

impl ClipKind {
    /// kindsだけを確定させて返す。propertiesはpendingのまま返し、まだ適用しない。
    /// 適用は他プラグイン分も含めて全kindsが出揃った後、`resolve_properties` で行う。
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
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(|e| {
                log::error!("Failed to parse properties in {}: {}", plugin_id, e);
                e
            })?;

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
                properties: raw
                    .properties
                    .iter()
                    .map(|id| {
                        let (owner, local) = id
                            .split_once(':')
                            .context("property reference must be namespaced")?;
                        NamespacedID::new(owner, local)
                    })
                    .collect::<anyhow::Result<_>>()?,
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

        if targets.is_empty() && prop.name.is_empty() {
            anyhow::bail!("property in `{plugin_id}` must have a name or target");
        }

        if !targets.is_empty() && !prop.name.is_empty() {
            anyhow::bail!("property must use either name or target, not both");
        }
        let names = prop
            .name
            .iter()
            .map(|name| {
                let (owner, local) = name
                    .split_once(':')
                    .context("property name must be namespaced")?;
                anyhow::ensure!(
                    owner == plugin_id,
                    "plugin `{plugin_id}` cannot define property `{name}`"
                );
                NamespacedID::new(owner, local)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let fields = prop
            .fields
            .into_iter()
            .map(|f| PropertySchema::parse_one(f, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        PropertySchema::validate_properties(&fields)?;
        Ok(PendingProperty {
            names,
            targets,
            fields,
        })
    }

    /// All named references pass through this boundary, including cross-plugin references.
    /// TODO: apply the host's permission policy here once plugin permissions are implemented.
    fn resolve_property_reference<'a>(
        kind: &NamespacedID,
        property: &NamespacedID,
        definitions: &HashMap<NamespacedID, &'a PendingProperty>,
    ) -> anyhow::Result<&'a PendingProperty> {
        definitions
            .get(property)
            .copied()
            .with_context(|| format!("property `{property}` referenced by `{kind}` not found"))
    }

    pub fn resolve_properties(
        kinds: &HashMap<NamespacedID, ClipKind>,
        pending: Vec<PendingProperty>,
    ) -> anyhow::Result<HashMap<NamespacedID, Vec<PropertySchema>>> {
        let mut index: HashMap<NamespacedID, Vec<PropertySchema>> = HashMap::new();

        // Resolve all definitions before references, independent of file/plugin load order.
        let mut definitions = HashMap::new();
        for p in &pending {
            for name in &p.names {
                anyhow::ensure!(
                    definitions.insert(name.clone(), p).is_none(),
                    "duplicate property definition `{name}`"
                );
            }
        }
        for (id, kind) in kinds {
            let mut seen = HashSet::new();
            for reference in &kind.properties {
                anyhow::ensure!(
                    seen.insert(reference),
                    "duplicate property reference `{reference}` in `{id}`"
                );
                let definition = Self::resolve_property_reference(id, reference, &definitions)?;
                index
                    .entry(id.clone())
                    .or_default()
                    .extend(definition.fields.clone());
            }
        }

        for p in &pending {
            if !p.names.is_empty() {
                continue;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(
        files: &[(&str, &str)],
    ) -> anyhow::Result<HashMap<NamespacedID, Vec<PropertySchema>>> {
        let mut kinds = HashMap::new();
        let mut pending = Vec::new();
        for (owner, text) in files {
            let (parsed, definitions) = ClipKind::parse_toml(text, owner)?;
            kinds.extend(parsed);
            pending.extend(definitions);
        }
        ClipKind::resolve_properties(&kinds, pending)
    }

    const KIND: &str = r#"
[[kinds]]
id = "video"
render_script = "render"
properties = ["std:video"]
"#;
    const DEFINITION: &str = r#"
[[properties]]
name = ["std:video"]
[[properties.fields]]
key = "source"
label = "Source"
category = ["Media"]
kind = { type = "string" }
"#;

    #[test]
    fn resolves_separate_files_and_cross_plugin_references() {
        let index = resolve(&[("std", KIND), ("std", DEFINITION)]).unwrap();
        assert_eq!(
            index[&NamespacedID::new("std", "video").unwrap()][0]
                .key
                .full(),
            "std:source"
        );
        let index = resolve(&[("other", KIND), ("std", DEFINITION)]).unwrap();
        assert_eq!(
            index[&NamespacedID::new("other", "video").unwrap()][0]
                .key
                .full(),
            "std:source"
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_foreign_definitions() {
        assert!(
            resolve(&[("std", KIND)])
                .unwrap_err()
                .to_string()
                .contains("not found")
        );
        assert!(
            resolve(&[("std", DEFINITION), ("std", DEFINITION)])
                .unwrap_err()
                .to_string()
                .contains("duplicate property definition")
        );
        assert!(ClipKind::parse_toml(DEFINITION, "other").is_err());
        let duplicate = KIND.replace(
            "properties = [\"std:video\"]",
            "properties = [\"std:video\", \"std:video\"]",
        );
        assert!(resolve(&[("std", &duplicate), ("std", DEFINITION)]).is_err());
        let collision = DEFINITION.replace("name = [\"std:video\"]", "name = [\"std:another\"]");
        let two = KIND.replace(
            "properties = [\"std:video\"]",
            "properties = [\"std:video\", \"std:another\"]",
        );
        assert!(
            resolve(&[("std", &two), ("std", DEFINITION), ("std", &collision)])
                .unwrap_err()
                .to_string()
                .contains("duplicate property key")
        );
    }

    #[test]
    fn preserves_legacy_targets() {
        let definition = DEFINITION.replace("name = [\"std:video\"]", "target = [\"std:video\"]");
        let kind = KIND.replace("properties = [\"std:video\"]", "");
        assert_eq!(
            resolve(&[("std", &kind), ("std", &definition)])
                .unwrap()
                .len(),
            1
        );
    }
}
