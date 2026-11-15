use super::*;

#[test]
fn parse_namespaced_root() {
    let path = PropertyPath::parse("std:video/source").unwrap();
    assert_eq!(path.root().full(), "std:video");
    assert_eq!(path.members(), &["source"]);
    assert_eq!(path.to_string(), "std:video/source");
}

#[test]
fn parse_rejects_root_without_namespace() {
    assert!(PropertyPath::parse("video/source").is_err());
}

#[test]
fn child_appends_member() {
    let path = PropertyPath::parse("std:video").unwrap();
    let child = path.child("transform").unwrap();
    assert_eq!(child.root().full(), "std:video");
    assert_eq!(child.members(), &["transform"]);
    assert_eq!(child.to_string(), "std:video/transform");
}

#[test]
fn nested_children_display() {
    let path = PropertyPath::parse("std:video")
        .unwrap()
        .child("transform")
        .unwrap()
        .child("position")
        .unwrap();
    assert_eq!(path.to_string(), "std:video/transform/position");
    assert_eq!(path.members(), &["transform", "position"]);
}
