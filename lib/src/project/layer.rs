use rkyv::{CheckBytes, bytecheck};
use std::collections::BTreeMap;

use crate::project::ids::{ClipId, LayerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerRemoveStrategy {
    Recursive,
    PromoteChildren,
}

/// レイヤーには特定の役割を持たせない(Video/Audio/Effectで型を分けない)。
/// children があれば Folder として振る舞う。実行時にフラット化するかは
/// executor 側の責務で、データ構造上は葉レイヤーと区別しない。
#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde:: Deserialize,
    Debug,
    Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub enabled: bool,
    /// position -> ClipId。Clip本体はここでは持たない(Timeline.clipsが実体)。
    pub clips: BTreeMap<i64, ClipId>,
}

impl Layer {
    pub fn new(id: LayerId, name: String) -> Self {
        Self {
            id,
            name,
            enabled: true,
            clips: BTreeMap::new(),
        }
    }

    pub fn get_clip_id_at(&self, pos: i64) -> Option<ClipId> {
        self.clips.range(..=pos).next_back().map(|(_, &id)| id)
    }

    /// clip_idの参照をこのレイヤーから除去し、あった位置を返す。
    /// Clip実体はTimeline.clipsが持つので、そちらの削除は呼び出し側の責務。
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

/// ProjectAll等で「構造だけ」送るための軽量版。clipsを含まない。
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
