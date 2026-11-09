use std::collections::HashMap;

use crate::{
    plugin::{NamespacedID, registry::PluginDefinitionRegistry},
    project::clip::ClipKind,
};

pub mod component;
pub mod parse;

/// 複数プラグインから合成した「どんな `ClipKind` が存在しうるか」の一覧です。
/// `ToolbarRegistry` / `SchemaRegistry` と対になります。
///
/// 各プラグインが `clips/` ディレクトリ下の TOML でクリップ種別を宣言し、
/// ここで重複チェック付きでマージします。
#[derive(Debug, Default)]
pub struct ClipKindRegistry {
    inner: PluginDefinitionRegistry<NamespacedID, ClipKind>,
}

impl ClipKindRegistry {
    /// 指定 ID の `ClipKind` への参照を返します。存在しなければ `None`。
    pub fn get(&self, id: &NamespacedID) -> Option<&ClipKind> {
        self.inner.get(id)
    }

    /// 全エントリへのイテレータを返します。
    pub fn iter(&self) -> impl Iterator<Item = (&NamespacedID, &ClipKind)> {
        self.inner.iter()
    }

    /// 指定 ID のエントリが存在するかを返します。
    pub fn contains(&self, id: &NamespacedID) -> bool {
        self.inner.contains(id)
    }

    /// プラグインが宣言するクリップ種別をマージします。
    /// ID 衝突があればエラーとなります。
    pub fn merge_plugin_kinds(
        &mut self,
        plugin_kinds: HashMap<NamespacedID, ClipKind>,
    ) -> anyhow::Result<()> {
        self.inner.merge(plugin_kinds)
    }
}

/// `ClipKind` の実体を保持する層です。
///
/// 現在はレイアウト（並び順）などのユーザー設定を持たず、`ToolbarStore` と
/// 対になる位置づけとして分離しています。
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

    /// プラグインが宣言するクリップ種別をマージします。
    pub fn merge_plugin_kinds(
        &mut self,
        plugin_kinds: HashMap<NamespacedID, ClipKind>,
    ) -> anyhow::Result<()> {
        self.registry.merge_plugin_kinds(plugin_kinds)
    }

    /// 指定 ID の `ClipKind` への参照を返します。
    pub fn get(&self, id: &NamespacedID) -> Option<&ClipKind> {
        self.registry.get(id)
    }
}
