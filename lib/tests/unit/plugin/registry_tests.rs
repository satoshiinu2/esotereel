use super::*;

#[test]
fn default_is_empty() {
    let reg: PluginDefinitionRegistry<String, i32> = PluginDefinitionRegistry::default();
    assert!(reg.entries.is_empty());
    assert_eq!(reg.iter().count(), 0);
}

#[test]
fn get_and_contains() {
    let mut reg: PluginDefinitionRegistry<String, i32> = PluginDefinitionRegistry::default();
    assert!(!reg.contains("a"));
    reg.merge(HashMap::from([("a".to_string(), 1)])).unwrap();
    assert!(reg.contains("a"));
    assert_eq!(reg.get("a"), Some(&1));
    assert_eq!(reg.get("missing"), None);
}

#[test]
fn merge_rejects_duplicate_id() {
    let mut reg: PluginDefinitionRegistry<String, i32> = PluginDefinitionRegistry::default();
    reg.merge(HashMap::from([("a".to_string(), 1)])).unwrap();
    let err = reg
        .merge(HashMap::from([("a".to_string(), 2)]))
        .unwrap_err();
    assert!(err.to_string().contains("duplicate id"));
}

#[test]
fn merge_adds_multiple_entries() {
    let mut reg: PluginDefinitionRegistry<String, i32> = PluginDefinitionRegistry::default();
    reg.merge(HashMap::from([("a".to_string(), 1), ("b".to_string(), 2)]))
        .unwrap();
    assert_eq!(reg.iter().count(), 2);
    assert_eq!(reg.get("b"), Some(&2));
}
