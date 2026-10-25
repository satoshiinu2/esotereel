use crate::plugin::NamespacedID;
use rkyv::{Archive, CheckBytes, bytecheck};
use std::fmt;

/// A property root and its local member names. Labels never form part of this identity.
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
    root: NamespacedID,
    members: Vec<String>,
}

impl PropertyPath {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let mut parts = text.split('/');
        let root = parts.next().unwrap_or_default();
        let (owner, local) = root
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("property path needs a namespaced root: {text}"))?;
        let mut path = Self::from(NamespacedID::new(owner, local)?);
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
    pub fn root(&self) -> &NamespacedID {
        &self.root
    }
    pub fn members(&self) -> &[String] {
        &self.members
    }
}
impl From<NamespacedID> for PropertyPath {
    fn from(root: NamespacedID) -> Self {
        Self {
            root,
            members: Vec::new(),
        }
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
