use std::{
    borrow::Borrow,
    collections::HashMap,
    hash::Hash,
};

/// プラグインから供給される定義(ClipKind, CompiledScript, etc.)を
/// id衝突チェック付きで集約する共通コンテナ。
#[derive(Debug)]
pub struct PluginDefinitionRegistry<K, V> {
    entries: HashMap<K, V>,
}

impl<K: Hash + Eq + std::fmt::Display + Clone, V> Default for PluginDefinitionRegistry<K, V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl<K: Hash + Eq + std::fmt::Display + Clone, V> PluginDefinitionRegistry<K, V> {
    pub fn get<Q: ?Sized + Hash + Eq>(&self, id: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.entries.get(id)
    }

    pub fn contains<Q: ?Sized + Hash + Eq>(&self, id: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.entries.contains_key(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter()
    }

    /// 既存(組み込み/他プラグイン)とid衝突があればエラーにして合流させる。
    pub fn merge(&mut self, incoming: HashMap<K, V>) -> anyhow::Result<()> {
        for id in incoming.keys() {
            if self.entries.contains_key(id) {
                anyhow::bail!("duplicate id `{}`", id);
            }
        }
        self.entries.extend(incoming);
        Ok(())
    }
}
