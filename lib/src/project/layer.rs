use rkyv::{CheckBytes, bytecheck};
use std::collections::BTreeMap;

use crate::project::ids::{ClipId, LayerId};

/// レイヤー削除時の子ノード処理方針です。
pub enum LayerRemoveStrategy {
    /// 子ノードも再帰的に削除します。
    Recursive,
    /// 子ノードを親に昇格させます。
    PromoteChildren,
}

/// ブレンドモードです。
#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Copy,
    PartialEq,
    Debug,
    Default,
)]
#[archive(check_bytes)]
pub enum BlendMode {
    #[default]
    Normal,
    Add,
    Multiply,
    Screen,
}

/// レイヤーの実体です。
///
/// `position -> ClipId` のマップを保持しますが、クリップ実体は
/// `Timeline.clips` が持つため、ここでは参照のみを管理します。
#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub enabled: bool,
    /// 位置 → `ClipId` のマップ。クリップ実体は `Timeline.clips` が保持。
    pub clips: BTreeMap<i64, ClipId>,
}

impl Layer {
    /// 新しいレイヤーを作成します。
    pub fn new(id: LayerId, name: String) -> Self {
        Self {
            id,
            name,
            enabled: true,
            clips: BTreeMap::new(),
        }
    }

    /// 指定位置にお or その直前にあるクリップ ID を返します。
    pub fn get_clip_id_at(&self, pos: i64) -> Option<ClipId> {
        self.clips.range(..=pos).next_back().map(|(_, &id)| id)
    }

    /// `clip_id` の参照をこのレイヤーから除去し、あった位置を返します。
    ///
    /// クリップ実体は `Timeline.clips` が持つため、そちらの削除は呼び出し側の責務です。
    pub fn remove_clip(&mut self, clip_id: ClipId) -> Option<i64> {
        let pos = self
            .clips
            .iter()
            .find(|(_, id)| **id == clip_id)
            .map(|(&p, _)| p)?;
        self.clips.remove(&pos);
        Some(pos)
    }
}

/// `ProjectAll` 等で「構造だけ」を送るための軽量版です。クリップを含みません。
#[derive(rkyv::Archive, rkyv::Serialize, serde::Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct LayerMeta {
    pub id: LayerId,
    pub name: String,
    pub enabled: bool,
}

impl From<&Layer> for LayerMeta {
    fn from(l: &Layer) -> Self {
        Self {
            id: l.id,
            name: l.name.clone(),
            enabled: l.enabled,
        }
    }
}
