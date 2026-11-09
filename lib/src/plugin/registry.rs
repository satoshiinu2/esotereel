use std::{borrow::Borrow, collections::HashMap, hash::Hash};

/// プラグインから供給される定義（`ClipKind`, `CompiledScript` など）を、
/// ID 衝突チェック付きで集約する共通コンテナです。
///
/// `K` は ID 型（`NamespacedID` など）、`V` は定義の型です。
/// 組み込み／他プラグインと ID が衝突する場合は `merge` でエラーを返し、
/// どのプラグインがどの ID を所有するかを明確にします。
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
    /// 指定 ID のエントリへの参照を返します。存在しなければ `None`。
    pub fn get<Q: ?Sized + Hash + Eq>(&self, id: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.entries.get(id)
    }

    /// 指定 ID のエントリが存在するかを返します。
    pub fn contains<Q: ?Sized + Hash + Eq>(&self, id: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.entries.contains_key(id)
    }

    /// 全エントリへのイテレータを返します。
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter()
    }

    /// 入力をマージします。既存エントリと ID が衝突すればエラーを返します。
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let reg: PluginDefinitionRegistry<String, i32> = PluginDefinitionRegistry::default();
        assert!(reg.entries.is_empty());
        assert_eq!(reg.iter().count(), 0);
    }

    #[test]
    fn get_and_contains() {
        let mut reg: PluginDefinitionRegistry<String, i32> =
            PluginDefinitionRegistry::default();
        assert!(!reg.contains("a"));
        reg.merge(HashMap::from([("a".to_string(), 1)])).unwrap();
        assert!(reg.contains("a"));
        assert_eq!(reg.get("a"), Some(&1));
        assert_eq!(reg.get("missing"), None);
    }

    #[test]
    fn merge_rejects_duplicate_id() {
        let mut reg: PluginDefinitionRegistry<String, i32> =
            PluginDefinitionRegistry::default();
        reg.merge(HashMap::from([("a".to_string(), 1)])).unwrap();
        let err = reg
            .merge(HashMap::from([("a".to_string(), 2)]))
            .unwrap_err();
        assert!(err.to_string().contains("duplicate id"));
    }

    #[test]
    fn merge_adds_multiple_entries() {
        let mut reg: PluginDefinitionRegistry<String, i32> =
            PluginDefinitionRegistry::default();
        reg.merge(HashMap::from([
            ("a".to_string(), 1),
            ("b".to_string(), 2),
        ]))
        .unwrap();
        assert_eq!(reg.iter().count(), 2);
        assert_eq!(reg.get("b"), Some(&2));
    }
}
