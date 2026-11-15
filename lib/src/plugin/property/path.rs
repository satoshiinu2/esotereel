use std::fmt;

use rkyv::{Archive, CheckBytes, bytecheck};

use crate::plugin::{NamespacedID, property::PropertyKey};

/// A property root and its local member names. Labels never form part of this identity.
///
/// 文字列表現は `plugin:component#property/member...` です。ルートがプロパティ
/// （`PropertyKey`）、メンバーが値マップ内のローカル名で、`/` で連結します。
/// 例: `std:media#media/source`、`std:transform#transform/position`。
#[derive(
    PartialEq,
    Eq,
    Debug,
    Clone,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[archive_attr(derive(CheckBytes))]
pub struct PropertyPath {
    root: PropertyKey,
    members: Vec<String>,
}

impl PropertyPath {
    /// 絶対形のアドレスをパースします。
    ///
    /// 所有者を省略した相対形（`#property/...`）は受理しません。文脈となる
    /// 所有者がある場所では `parse_in_context` を使ってください。
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        Self::parse_with(text, None)
    }

    /// 文脈となる所有者を補ってアドレスをパースします。
    ///
    /// 所有者を省略した相対形（`#property/...`）も受理します。
    pub fn parse_in_context(text: &str, owner: &NamespacedID) -> anyhow::Result<Self> {
        Self::parse_with(text, Some(owner))
    }

    fn parse_with(text: &str, owner: Option<&NamespacedID>) -> anyhow::Result<Self> {
        let mut parts = text.split('/');
        let root_text = parts.next().unwrap_or_default();
        let root = match owner {
            Some(owner) => PropertyKey::parse_in_context(root_text, owner)?,
            None => PropertyKey::parse(root_text)?,
        };
        let mut path = Self::from(root);
        for member in parts {
            path = path.child(member)?;
        }
        Ok(path)
    }

    pub fn child(&self, member: &str) -> anyhow::Result<Self> {
        NamespacedID::new("member", member)?;
        let mut result = self.clone();
        result.members.push(member.to_owned());
        Ok(result)
    }

    pub fn root(&self) -> &PropertyKey {
        &self.root
    }

    pub fn members(&self) -> &[String] {
        &self.members
    }
}

impl From<PropertyKey> for PropertyPath {
    fn from(root: PropertyKey) -> Self {
        Self {
            root,
            members: Vec::new(),
        }
    }
}

impl From<NamespacedID> for PropertyPath {
    fn from(root: NamespacedID) -> Self {
        Self::from(PropertyKey::from_namespaced(root))
    }
}

impl fmt::Display for PropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)?;
        for member in &self.members {
            write!(f, "/{member}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
        assert!(PropertyPath::parse("#media/source").is_err());
    }

    #[test]
    fn parse_in_context_accepts_relative_form() {
        let owner = NamespacedID::new("std", "media").unwrap();
        let path = PropertyPath::parse_in_context("#media/source", &owner).unwrap();
        assert_eq!(path.to_string(), "std:media#media/source");
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
}
