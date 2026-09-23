use std::collections::HashMap;

use crate::{
    plugin::{NamespacedID, registry::PluginDefinitionRegistry},
    project::clip::ClipKind,
};

pub mod parse;

pub use parse::{PendingProperty, Target};

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
