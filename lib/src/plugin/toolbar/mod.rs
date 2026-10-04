use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use anyhow::Context;

use crate::plugin::{NamespacedID, toolbar::parse::ToolbarButtonRaw};

mod parse;

/// ボタンを押した時に何をするか。
/// Builtinはネイティブ(Qt/C++)側で実装済みの操作(zoom等)を指す安定id、
/// Scriptはプラグインのエントリポイント関数名を指す。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ToolbarAction {
    pub func_name: String,
    #[serde(default)]
    pub run_on: RunOn,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunOn {
    Client,
    Server,
}

impl Default for RunOn {
    fn default() -> Self {
        RunOn::Client
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ToolbarButtonSpec {
    pub id: NamespacedID,
    /// どのツールバーに属するか("timeline" 等)。将来複数ツールバーに対応するため。
    #[serde(default = "default_target")]
    pub target: String,
    pub label: String,
    #[serde(default)]
    pub tooltip: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub action: ToolbarAction,
}

fn default_target() -> String {
    "timeline".to_string()
}

impl ToolbarButtonSpec {
    pub fn parse_toml(text: &str, plugin_id: &str) -> anyhow::Result<Vec<ToolbarButtonSpec>> {
        let parsed: ToolbarFile = toml::from_str(text).map_err(|e| {
            anyhow::anyhow!("failed to parse toolbars TOML in {}: {}", plugin_id, e)
        })?;

        let buttons = parsed
            .buttons
            .into_iter()
            .map(|raw| Self::parse_button(raw, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Self::validate_buttons(&buttons)
            .with_context(|| format!("toolbar schema validation failed in `{plugin_id}`"))?;

        Ok(buttons)
    }

    fn parse_button(raw: ToolbarButtonRaw, plugin_id: &str) -> anyhow::Result<ToolbarButtonSpec> {
        let action = ToolbarAction::from_toml_value(&raw.action).with_context(|| {
            format!("invalid `action` for button `{}` in `{plugin_id}`", raw.id)
        })?;

        Ok(ToolbarButtonSpec {
            id: NamespacedID::new(plugin_id, &raw.id)?,
            target: raw.target.unwrap_or_else(default_target),
            label: raw.label,
            tooltip: raw.tooltip.unwrap_or_default(),
            icon: raw.icon,
            action,
        })
    }

    fn validate_buttons<'a>(
        buttons: impl IntoIterator<Item = &'a ToolbarButtonSpec>,
    ) -> anyhow::Result<()> {
        let mut seen = std::collections::HashSet::new();
        for button in buttons {
            if !seen.insert(button.id.clone()) {
                anyhow::bail!("duplicate toolbar button id `{}` ", button.id,);
            }
            // maybe not needed with icon button
            // if button.label.trim().is_empty() {
            //     anyhow::bail!("toolbar button `{}` has an empty label", button.id);
            // }
        }
        Ok(())
    }
}

#[derive(Debug, serde::Deserialize)]
struct ToolbarFile {
    buttons: Vec<ToolbarButtonRaw>,
}

/// 「どんなボタンが存在しうるか」の一覧(組み込み + プラグイン由来を合成したもの)。
/// settings::SchemaRegistry と対になる。
#[derive(Debug, Default)]
pub struct ToolbarRegistry {
    // button id -> button spec
    buttons: HashMap<NamespacedID, ToolbarButtonSpec>,
}

impl ToolbarRegistry {
    pub fn buttons(&self) -> impl Iterator<Item = &ToolbarButtonSpec> {
        self.buttons.values()
    }

    pub fn buttons_for_target(&self, target: &str) -> impl Iterator<Item = &ToolbarButtonSpec> {
        self.buttons().filter(move |b| b.target == target)
    }

    pub fn get_button_by(&self, id: &NamespacedID) -> Option<&ToolbarButtonSpec> {
        self.buttons.get(id)
    }

    pub fn from_plugin_buttons(plugin_buttons: Vec<(String, ToolbarButtonSpec)>) -> Self {
        let buttons: HashMap<NamespacedID, ToolbarButtonSpec> = plugin_buttons
            .into_iter()
            .map(|(_, spec)| (spec.id.clone(), spec))
            .collect();
        Self { buttons }
    }
}

#[derive(Debug, Default)]
pub struct ToolbarStore {
    pub registry: ToolbarRegistry,
    // target名 -> 有効なボタンidの並び順
    layouts: HashMap<String, Vec<NamespacedID>>,
    has_disk_loaded: bool,
}

impl ToolbarStore {
    pub fn new() -> Self {
        Self {
            registry: ToolbarRegistry::default(),
            layouts: HashMap::new(),
            has_disk_loaded: false,
        }
    }

    pub fn from_plugin_buttons(plugin_buttons: Vec<(String, ToolbarButtonSpec)>) -> Self {
        Self {
            registry: ToolbarRegistry::from_plugin_buttons(plugin_buttons),
            layouts: HashMap::new(),
            has_disk_loaded: false,
        }
    }

    /// レイアウト未設定のtargetに、レジストリ登録順のデフォルトを埋める。
    /// SettingsStore::add_missing_from_schema と同じ役割。
    pub fn fill_missing_from_registry(&mut self) {
        let mut targets: std::collections::HashSet<String> =
            self.registry.buttons().map(|b| b.target.clone()).collect();
        targets.extend(self.layouts.keys().cloned());

        for target in targets {
            if self.layouts.contains_key(&target) {
                continue;
            }
            // registryの借用はここで終わらせてからinsertする(借用を跨がせない)
            let default_ids: Vec<NamespacedID> = self
                .registry
                .buttons_for_target(&target)
                .into_iter()
                .map(|b| b.id.clone())
                .collect();
            self.layouts.insert(target, default_ids);
        }
    }

    /// ディスクから読み込めた分だけ上書き。存在しないtargetはデフォルトのまま残る。
    pub fn apply_loaded_layout(&mut self, loaded: HashMap<String, Vec<NamespacedID>>) {
        for (target, ids) in loaded {
            self.layouts.insert(target, ids);
        }
        self.has_disk_loaded = true;
    }

    /// レイアウト未設定ならレジストリの登録順にフォールバックした状態で返す。
    pub fn get_layout(&self, target: &str) -> Vec<NamespacedID> {
        match self.layouts.get(target) {
            Some(ids) => ids.clone(),
            None => self
                .registry
                .buttons_for_target(target)
                .into_iter()
                .map(|b| b.id.clone())
                .collect(),
        }
    }

    /// registryに存在しないidは黙って弾く。
    pub fn set_layout(&mut self, target: String, ordered_ids: Vec<NamespacedID>) {
        let known: HashSet<NamespacedID> = self
            .registry
            .buttons_for_target(&target)
            .into_iter()
            .map(|b| b.id.clone())
            .collect();
        let filtered: Vec<NamespacedID> = ordered_ids
            .into_iter()
            .filter(|id| known.contains(id))
            .collect();
        self.layouts.insert(target, filtered);
    }
}

pub async fn load_toolbar_layout(path: &Path) -> anyhow::Result<HashMap<String, Vec<String>>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let text = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read toolbar layout file at {}", path.display()))?;
    let table: HashMap<String, Vec<String>> = toml::from_str(&text)
        .with_context(|| format!("failed to parse toolbar layout file at {}", path.display()))?;
    Ok(table)
}

pub async fn save_toolbar_layout(
    path: &Path,
    layouts: &HashMap<String, Vec<String>>,
) -> anyhow::Result<()> {
    let text = toml::to_string_pretty(layouts).context("failed to serialize toolbar layout")?;
    tokio::fs::write(path, text)
        .await
        .with_context(|| format!("failed to write toolbar layout file at {}", path.display()))?;
    Ok(())
}
