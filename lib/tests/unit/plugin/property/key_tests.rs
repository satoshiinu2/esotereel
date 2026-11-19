use super::*;

#[test]
fn component_key_address_roundtrip() {
    let key = PropertyKey::new(NamespacedID::new("std", "media").unwrap(), "media").unwrap();
    assert_eq!(key.address(), "std:media#media");
    assert_eq!(key.owner().full(), "std:media");
    assert_eq!(key.name(), "media");
    assert_eq!(key.local_name(), "media");
    assert_eq!(PropertyKey::parse("std:media#media").unwrap(), key);
}

#[test]
fn namespaced_key_keeps_plain_id_form() {
    let key = PropertyKey::from_namespaced(NamespacedID::new("core", "log.level").unwrap());
    assert_eq!(key.address(), "core:log.level");
    assert_eq!(key.name(), "");
    assert_eq!(key.local_name(), "log.level");
    assert_eq!(PropertyKey::parse("core:log.level").unwrap(), key);
}

#[test]
fn parse_splits_on_hash_before_namespaced_id() {
    // `#` 以降が NamespacedID のローカル ID に取り込まれないこと。
    let key = PropertyKey::parse("std:transform#transform").unwrap();
    assert_eq!(key.owner().full(), "std:transform");
    assert_eq!(key.name(), "transform");
}

#[test]
fn parse_in_context_accepts_relative_form() {
    let owner = NamespacedID::new("std", "media").unwrap();
    let key = PropertyKey::parse_in_context(":#media", &owner).unwrap();
    assert_eq!(key.address(), "std:media#media");
    // `:` を欠いた `#name` は相対形として受理しない。
    assert!(PropertyKey::parse_in_context("#media", &owner).is_err());
    // 絶対形もそのまま受理する。
    assert_eq!(
        PropertyKey::parse_in_context("std:transform#transform", &owner).unwrap(),
        PropertyKey::new(NamespacedID::new("std", "transform").unwrap(), "transform").unwrap()
    );
}

#[test]
fn invalid_local_names_are_rejected() {
    let owner = NamespacedID::new("std", "media").unwrap();
    assert!(PropertyKey::new(owner.clone(), "").is_err());
    assert!(PropertyKey::new(owner.clone(), "a:b").is_err());
    assert!(PropertyKey::new(owner.clone(), "a#b").is_err());
    assert!(PropertyKey::new(owner, "a/b").is_err());
    assert!(PropertyKey::parse("std:media#").is_err());
    assert!(PropertyKey::parse("#media").is_err());
}

#[test]
fn serde_uses_address_string() {
    let key = PropertyKey::new(NamespacedID::new("std", "media").unwrap(), "media").unwrap();
    let json = serde_json::to_string(&key).unwrap();
    assert_eq!(json, "\"std:media#media\"");
    assert_eq!(serde_json::from_str::<PropertyKey>(&json).unwrap(), key);
}
