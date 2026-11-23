use std::collections::HashMap;

use anyhow::Context;

use crate::{
    plugin::{
        clip::component::{
            ComponentCapability, ComponentDefinition, parse_component_definitions,
            resolve_namespaced,
        },
        NamespacedID,
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
    components: Vec<String>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct ClipKindFile {
    #[serde(default)]
    kinds: Vec<ClipKindRaw>,
}

/// ClipKind の実体を TOML テキストからパースする。
///
/// この関数は `[[kinds]]` セクションのみを扱う。コンポーネント定義は
/// `components/` ディレクトリ下の別 TOML ファイルから `parse_component_definitions`
/// で個別に読み込む。クリップ側のプロパティ宣言（`[[properties]]`）は廃止され、
/// プロパティは各コンポーネントが自身のスキーマとして提供する。
impl ClipKind {
    pub fn parse_toml(
        text: &str,
        plugin_id: &str,
    ) -> anyhow::Result<HashMap<NamespacedID, ClipKind>> {
        let parsed: ClipKindFile = toml::from_str(text).map_err(|e| {
            anyhow::anyhow!("failed to parse clip kinds TOML in {}: {}", plugin_id, e)
        })?;

        Self::parse_kinds(parsed.kinds, plugin_id)
    }

    fn parse_kinds(
        raw_kinds: Vec<ClipKindRaw>,
        plugin_id: &str,
    ) -> anyhow::Result<HashMap<NamespacedID, ClipKind>> {
        let mut kinds = HashMap::new();
        for raw in raw_kinds {
            let id = NamespacedID::new(plugin_id, &raw.id)?;
            let mut components = Vec::new();
            for component in &raw.components {
                components.push(resolve_namespaced(component, plugin_id).with_context(|| {
                    format!(
                        "invalid component id `{component}` in clip kind `{}`",
                        id.full()
                    )
                })?);
            }
            let kind = ClipKind {
                render_script: raw.render_script,
                tags: raw.tags.into_iter().collect(),
                components,
            };
            if kinds.insert(id.clone(), kind).is_some() {
                anyhow::bail!("duplicate clip kind id `{}`", id);
            }
        }
        Ok(kinds)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/plugin/clip/parse_tests.rs"]
mod tests;
