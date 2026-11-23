use super::*;

#[test]
fn parse_canonical_address() {
    let path = PropertyPath::parse("std:media#media/source").unwrap();
    assert_eq!(path.root().address(), "std:media#media");
    assert_eq!(path.members(), &["source"]);
    assert_eq!(path.to_string(), "std:media#media/source");
}

#[test]
fn parse_namespaced_root() {
    let path = PropertyPath::parse("test:text/content").unwrap();
    assert_eq!(path.root().address(), "test:text");
    assert_eq!(path.members(), &["content"]);
    assert_eq!(path.to_string(), "test:text/content");
}

#[test]
fn parse_rejects_relative_form_without_context() {
    assert!(PropertyPath::parse(":#media/source").is_err());
    assert!(PropertyPath::parse("#media/source").is_err());
}

#[test]
fn parse_in_context_accepts_relative_form() {
    let owner = NamespacedID::new("std", "media").unwrap();
    let path = PropertyPath::parse_in_context(":#media/source", &owner).unwrap();
    assert_eq!(path.to_string(), "std:media#media/source");
    // `:` を欠いた `#property/...` は相対形として受理しない。
    assert!(PropertyPath::parse_in_context("#media/source", &owner).is_err());
    // 絶対形もそのまま受理する。
    let absolute =
        PropertyPath::parse_in_context("std:transform#transform/position", &owner).unwrap();
    assert_eq!(absolute.to_string(), "std:transform#transform/position");
}

#[test]
fn parse_rejects_root_without_namespace() {
    assert!(PropertyPath::parse("video/source").is_err());
}

#[test]
fn parse_rejects_empty_member() {
    assert!(PropertyPath::parse("std:media#media//source").is_err());
}

#[test]
fn child_appends_member() {
    let path = PropertyPath::parse("std:transform#transform").unwrap();
    let child = path.child("position").unwrap();
    assert_eq!(child.root().address(), "std:transform#transform");
    assert_eq!(child.members(), &["position"]);
    assert_eq!(child.to_string(), "std:transform#transform/position");
}

#[test]
fn nested_children_display() {
    let path = PropertyPath::parse("std:transform#transform")
        .unwrap()
        .child("position")
        .unwrap()
        .child("x")
        .unwrap();
    assert_eq!(path.to_string(), "std:transform#transform/position/x");
    assert_eq!(path.members(), &["position", "x"]);
}
