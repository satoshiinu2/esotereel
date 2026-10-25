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
    label: Option<String>,
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
    pub fields: Vec<PropertySchemaRaw>,
    pub plugin_id: String,
    pub label: Option<String>,
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
        Ok(PendingProperty {
            names,
            targets,
            fields: prop.fields,
            plugin_id: plugin_id.to_owned(),
            label: prop.label,
        })
    }

    /// All references pass through this boundary. Future permission checks belong here.
    fn resolve_property_reference<'a>(
        requester: &NamespacedID,
        property: &NamespacedID,
        definitions: &HashMap<NamespacedID, &'a PendingProperty>,
    ) -> anyhow::Result<&'a PendingProperty> {
        definitions
            .get(property)
            .copied()
            .with_context(|| format!("property `{property}` referenced by `{requester}` not found"))
    }

    fn resolve_group(
        requester: &NamespacedID,
        reference: &NamespacedID,
        definitions: &HashMap<NamespacedID, &PendingProperty>,
        stack: &mut Vec<NamespacedID>,
    ) -> anyhow::Result<PropertySchema> {
        anyhow::ensure!(
            !stack.contains(reference),
            "cyclic property reference: {} -> {reference}",
            stack
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" -> ")
        );
        anyhow::ensure!(
            stack.len() < 32,
            "property nesting exceeds 32 groups at `{reference}`"
        );
        let definition = Self::resolve_property_reference(requester, reference, definitions)?;
        stack.push(reference.clone());
        let fields = Self::resolve_fields(definition, reference, definitions, stack)?;
        stack.pop();
        let group = PropertySchema::group(
            reference.clone(),
            definition
                .label
                .clone()
                .unwrap_or_else(|| reference.local_id().to_owned()),
            reference.clone(),
            fields,
        );
        PropertySchema::validate_properties(std::slice::from_ref(&group))?;
        Ok(group)
    }

    fn resolve_fields(
        definition: &PendingProperty,
        requester: &NamespacedID,
        definitions: &HashMap<NamespacedID, &PendingProperty>,
        stack: &mut Vec<NamespacedID>,
    ) -> anyhow::Result<Vec<PropertySchema>> {
        let mut fields = Vec::new();
        for raw in &definition.fields {
            if raw.kind.get("type").and_then(toml::Value::as_str) == Some("group") {
                let reference = raw
                    .kind
                    .get("reference")
                    .and_then(toml::Value::as_str)
                    .context("group kind requires a namespaced `reference`")?;
                let reference = parse_id(reference)?;
                let mut group = Self::resolve_group(requester, &reference, definitions, stack)?;
                group.key = NamespacedID::new(&definition.plugin_id, &raw.key)?;
                group.label = raw.label.clone();
                group.category = raw.category.clone();
                if let Some(value) = &raw.default {
                    group.default = group.kind.parse_toml_to(value)?;
                }
                fields.push(group);
            } else {
                fields.push(PropertySchema::parse_one(
                    raw.clone(),
                    &definition.plugin_id,
                )?);
            }
        }
        PropertySchema::validate_properties(&fields)?;
        Ok(fields)
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
        // Validate unused definitions too, so broken references never depend on usage order.
        for reference in definitions.keys() {
            Self::resolve_group(reference, reference, &definitions, &mut Vec::new())?;
        }
        for (id, kind) in kinds {
            let mut seen = HashSet::new();
            for reference in &kind.properties {
                anyhow::ensure!(
                    seen.insert(reference),
                    "duplicate property reference `{reference}` in `{id}`"
                );
                let group = Self::resolve_group(id, reference, &definitions, &mut Vec::new())?;
                index.entry(id.clone()).or_default().push(group);
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
                let fields = Self::resolve_fields(p, &id, &definitions, &mut Vec::new())?;
                index.entry(id).or_default().extend(fields);
            }
        }

        for fields in index.values() {
            PropertySchema::validate_properties(fields)?;
        }

        Ok(index)
    }
}

fn parse_id(text: &str) -> anyhow::Result<NamespacedID> {
    let (owner, local) = text
        .split_once(':')
        .context("reference must be namespaced")?;
    NamespacedID::new(owner, local)
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
            "std:video"
        );
        let index = resolve(&[("other", KIND), ("std", DEFINITION)]).unwrap();
        assert_eq!(
            index[&NamespacedID::new("other", "video").unwrap()][0]
                .key
                .full(),
            "std:video"
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
        let duplicate_field = format!(
            r#"{DEFINITION}
[[properties.fields]]
key = "source"
label = "Duplicate"
kind = {{ type = "string" }}
"#
        );
        assert!(resolve(&[("std", KIND), ("std", &duplicate_field)]).is_err());
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
    #[test]
    fn nested_references_keep_identity_defaults_and_independent_instances() {
        let text = r#"
[[properties]]
name = ["text:style"]
label = "Style"
[[properties.fields]]
key = "font_size"
label = "Font Size"
kind = { type = "float", step = 1.0 }
default = 24.0
[[properties]]
name = ["text:text"]
label = "Text"
[[properties.fields]]
key = "content"
label = "Content"
kind = { type = "string" }
[[properties.fields]]
key = "style"
label = "Style"
kind = { type = "group", reference = "text:style" }
"#;
        let telop = r#"
[[kinds]]
id = "telop"
render_script = "render"
properties = ["generator:telop"]
[[properties]]
name = ["generator:telop"]
label = "Telop"
[[properties.fields]]
key = "caption"
label = "Caption"
kind = { type = "group", reference = "text:text" }
default = { content = "Caption", style = { font_size = 36.0 } }
[[properties.fields]]
key = "header"
label = "Header"
kind = { type = "group", reference = "text:text" }
default = { content = "Header" }
"#;
        let resolved = resolve(&[("generator", telop), ("text", text)]).unwrap();
        let schema = &resolved[&NamespacedID::new("generator", "telop").unwrap()];
        let crate::plugin::property::value::FieldTypeKind::Group { fields, .. } = &schema[0].kind
        else {
            panic!("group")
        };
        let crate::plugin::property::value::FieldTypeKind::Group { reference, .. } =
            &fields[0].kind
        else {
            panic!("nested group")
        };
        assert_eq!(reference.full(), "text:text");
        let leaves = PropertySchema::leaf_fields(schema).unwrap();
        assert_eq!(
            leaves
                .iter()
                .map(|(p, _)| p.to_string())
                .collect::<Vec<_>>(),
            [
                "generator:telop/caption/content",
                "generator:telop/caption/style/font_size",
                "generator:telop/header/content",
                "generator:telop/header/style/font_size"
            ]
        );
        use crate::plugin::property::value::FieldValue;
        assert_eq!(leaves[1].1.default, FieldValue::Float(36.0));
        assert_eq!(leaves[3].1.default, FieldValue::Float(24.0));
        assert_eq!(leaves[1].1.category, ["Telop", "Caption", "Style"]);
        let defaults = crate::project::clip::ClipBindingValue::default_properties(schema);
        let mut clip = crate::project::Clip::new(
            1,
            0,
            60,
            NamespacedID::new("generator", "telop").unwrap(),
            defaults,
            crate::project::transform::ClipTranslates::None,
        );
        let caption =
            crate::plugin::property::PropertyPath::parse("generator:telop/caption/content")
                .unwrap();
        clip.set_property_path(
            &caption,
            crate::project::clip::ClipBindingValue::Static(FieldValue::String("Changed".into())),
        )
        .unwrap();
        assert_eq!(
            clip.get_property_path(
                &crate::plugin::property::PropertyPath::parse("generator:telop/header/content")
                    .unwrap()
            ),
            Some(&FieldValue::String("Header".into()))
        );
        let props = crate::plugin::script::bridge::field_values_to_rhai_map(&clip.properties);
        let mut scope = rhai::Scope::new();
        scope.push("props", props);
        let value = rhai::Engine::new()
            .eval_with_scope::<String>(&mut scope, r#"props["generator:telop"].caption.content"#)
            .unwrap();
        assert_eq!(value, "Changed");
        let bytes = rkyv::to_bytes::<_, 1024>(&clip).unwrap();
        let decoded = rkyv::from_bytes::<crate::project::Clip>(&bytes).unwrap();
        assert_eq!(
            decoded.get_property_path(&caption),
            Some(&FieldValue::String("Changed".into()))
        );

        let unknown_default = telop.replace("content = \"Caption\"", "unknown = \"Caption\"");
        assert!(resolve(&[("generator", &unknown_default), ("text", text)]).is_err());
    }

    #[test]
    fn rejects_cycles_and_missing_nested_references_even_when_unused() {
        let cyclic = r#"
[[properties]]
name = ["test:a"]
[[properties.fields]]
key = "b"
label = "B"
kind = { type = "group", reference = "test:b" }
[[properties]]
name = ["test:b"]
[[properties.fields]]
key = "a"
label = "A"
kind = { type = "group", reference = "test:a" }
"#;
        assert!(
            resolve(&[("test", cyclic)])
                .unwrap_err()
                .to_string()
                .contains("cyclic property reference")
        );
        let missing = cyclic.replace("reference = \"test:a\"", "reference = \"test:missing\"");
        assert!(
            resolve(&[("test", &missing)])
                .unwrap_err()
                .to_string()
                .contains("not found")
        );
    }
}
