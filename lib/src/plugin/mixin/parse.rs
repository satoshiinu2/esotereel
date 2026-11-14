use std::collections::HashSet;

use anyhow::Context;
use serde::Deserialize;

use crate::plugin::{
    mixin::{MixinAt, MixinDeclaration},
    property::ComponentCapability,
    NamespacedID,
};

/// `plugin:local` 形式、またはプラグイン下のローカル ID を NamespacedID に解決する。
fn resolve_namespaced(value: &str, plugin_id: &str) -> anyhow::Result<NamespacedID> {
    if value.contains(':') {
        NamespacedID::parse(value)
    } else {
        NamespacedID::new(plugin_id, value)
    }
}

/// TOML からパースするための中間表現。
#[derive(Debug, Deserialize)]
struct MixinDeclarationRaw {
    id: String,
    target_fn: String,
    #[serde(default)]
    target_component: Option<String>,
    #[serde(default)]
    target_capability: Option<String>,
    at: MixinAt,
    #[serde(default)]
    priority: Option<i64>,
    script: String,
    handler: String,
}

impl MixinDeclarationRaw {
    fn resolve(self, plugin_id: &str) -> anyhow::Result<MixinDeclaration> {
        let id = NamespacedID::new(plugin_id, &self.id).with_context(|| {
            format!(
                "invalid mixin declaration id `{}` in plugin `{}`",
                self.id, plugin_id
            )
        })?;
        let target_component = match &self.target_component {
            Some(value) => Some(resolve_namespaced(value, plugin_id).with_context(|| {
                format!(
                    "invalid target component `{value}` in mixin `{}`",
                    id.full()
                )
            })?),
            None => None,
        };
        let target_capability = match &self.target_capability {
            Some(value) => Some(value.parse::<ComponentCapability>().with_context(|| {
                format!("invalid target capability `{value}` in mixin `{}`", id.full())
            })?),
            None => None,
        };
        Ok(MixinDeclaration {
            id,
            target_fn: self.target_fn,
            target_component,
            target_capability,
            at: self.at,
            priority: self.priority,
            script: self.script,
            handler: self.handler,
        })
    }
}

/// TOML テキストから `[[mixins]]` 配列をパースし、宣言のリストを返す。
///
/// 同一ファイル内で宣言 ID が重複した場合はエラーにする。
pub fn parse_mixin_declarations(
    text: &str,
    plugin_id: &str,
) -> anyhow::Result<Vec<MixinDeclaration>> {
    #[derive(Debug, Deserialize, Default)]
    struct MixinsFile {
        #[serde(default)]
        mixins: Vec<MixinDeclarationRaw>,
    }

    let parsed: MixinsFile = toml::from_str(text).map_err(|e| {
        anyhow::anyhow!(
            "failed to parse mixin declarations TOML in {}: {}",
            plugin_id,
            e
        )
    })?;

    let mut declarations = Vec::new();
    let mut seen: HashSet<NamespacedID> = HashSet::new();
    for raw in parsed.mixins {
        let declaration = raw.resolve(plugin_id)?;
        if !seen.insert(declaration.id.clone()) {
            anyhow::bail!(
                "duplicate mixin declaration id `{}` in plugin `{plugin_id}`",
                declaration.id
            );
        }
        declarations.push(declaration);
    }
    Ok(declarations)
}
