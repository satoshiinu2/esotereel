use std::fmt;

use anyhow::Context;
use rkyv::{Archive, CheckBytes, bytecheck};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::plugin::NamespacedID;

/// プロパティの識別子です。所有者と、その中でのローカル名の組で表します。
///
/// 正規形（文字列表現）は `plugin:component#name` です。`#` より前が所有者
/// （コンポーネント等の `NamespacedID`）、後ろが所有者内でのローカル名です。
/// 例: `std:media#media` はコンポーネント `std:media` が宣言するプロパティ `media`。
///
/// ローカル名が空のキーは、プラグイン名前空間直下の識別子（設定スキーマ等）を
/// 表し、文字列表現は所有者の `NamespacedID` そのものになります
/// （例: `core:log.level`）。
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[archive_attr(derive(CheckBytes))]
pub struct PropertyKey {
    owner: NamespacedID,
    name: String,
}

impl PropertyKey {
    /// 所有者を持つプロパティのキーを作ります。
    ///
    /// `name` は所有者内でのローカル名で、アドレス文法の区切り（`:` `#` `/`）を
    /// 含められません。
    pub fn new(owner: NamespacedID, name: &str) -> anyhow::Result<Self> {
        validate_local_name(name)?;
        Ok(Self {
            owner,
            name: name.to_owned(),
        })
    }

    /// プラグイン名前空間直下の識別子（設定スキーマ等）をキーとして表します。
    pub fn from_namespaced(id: NamespacedID) -> Self {
        Self {
            owner: id,
            name: String::new(),
        }
    }

    /// アドレス文字列をパースします。
    ///
    /// 所有者とローカル名を分けるため、必ず `#` で先に割ってから所有者を
    /// `NamespacedID` として解釈します。
    ///
    /// 所有者を省略した相対形（`:#name`）は受理しません（所有者として
    /// 解釈される ":" が ID として不正なためエラーになります）。相対形には
    /// 文脈となる所有者を補う `parse_in_context` を使ってください。
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        match text.split_once('#') {
            Some((owner, name)) => Self::new(
                NamespacedID::parse(owner)
                    .with_context(|| format!("invalid property key `{text}`"))?,
                name,
            ),
            None => Ok(Self::from_namespaced(NamespacedID::parse(text)?)),
        }
    }

    /// 文脈となる所有者を補ってアドレス文字列をパースします。
    ///
    /// 所有者を省略した相対形 `:#name` は、ここでのみ受理します。先頭の `:`
    /// が「所有者が省略されている（＝文脈となる所有者自身）」ことの明示で、
    /// `:` を欠いた `#name` の形は受理しません。
    pub fn parse_in_context(text: &str, owner: &NamespacedID) -> anyhow::Result<Self> {
        match text.strip_prefix(":#") {
            Some(name) => Self::new(owner.clone(), name),
            None => Self::parse(text),
        }
    }

    /// 所有者の ID を返します。
    pub fn owner(&self) -> &NamespacedID {
        &self.owner
    }

    /// 所有者内でのローカル名を返します。名前空間直下のキーでは空文字列です。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// ローカル名を返します。名前空間直下のキーでは所有者のローカル ID を返します。
    ///
    /// グループのメンバー名など、値マップ内での名前が必要な場面で使います。
    pub fn local_name(&self) -> &str {
        if self.name.is_empty() {
            self.owner.local_id()
        } else {
            &self.name
        }
    }

    /// 名前空間直下のキーであれば、その `NamespacedID` を返します。
    pub fn as_namespaced(&self) -> Option<&NamespacedID> {
        self.name.is_empty().then_some(&self.owner)
    }

    /// 正規形のアドレス文字列を返します。
    pub fn address(&self) -> String {
        if self.name.is_empty() {
            self.owner.full().to_owned()
        } else {
            format!("{}#{}", self.owner.full(), self.name)
        }
    }
}

/// プロパティや依存のローカル名を検証します。
///
/// アドレス文法の区切り（`:` `#` `/`）と衝突しない文字種のみを許可します。
pub fn validate_local_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        anyhow::bail!(
            "invalid local name `{name}`: use ASCII letters, digits, '.', '_' or '-'"
        );
    }
    Ok(())
}

impl fmt::Display for PropertyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&self.address())
    }
}

// rkyv builds hash-map indices using the original key, then validates and looks up
// entries using the archived key. Both types must hash and compare the same fields.
impl PartialEq for ArchivedPropertyKey {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner && self.name == other.name
    }
}
impl Eq for ArchivedPropertyKey {}
impl std::hash::Hash for ArchivedPropertyKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.owner.hash(state);
        self.name.hash(state);
    }
}

impl Serialize for PropertyKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.address())
    }
}

impl<'de> Deserialize<'de> for PropertyKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/plugin/property/key_tests.rs"]
mod tests;
