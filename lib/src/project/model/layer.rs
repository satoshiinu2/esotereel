use rkyv::{CheckBytes, bytecheck};
use std::collections::BTreeMap;

use crate::{project::clip::Clip, util::result::EsotereelError};

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
pub struct LayerModel {
    pub id: u64,
    pub order: u32,
    pub name: String,
    pub clips: BTreeMap<i64, Clip>, // key: position
}

impl LayerModel {
    pub fn new(id: u64, order: u32, name: String) -> Self {
        Self {
            id,
            order,
            name,
            clips: BTreeMap::new(),
        }
    }

    pub fn try_insert(&mut self, new_clip: Clip) -> Result<(), EsotereelError> {
        let new_pos = new_clip.position();
        let new_end = new_pos + new_clip.duration;

        // 重複チェック
        for (pos, clip) in &self.clips {
            let clip_end = pos + clip.duration;
            if new_pos < clip_end && new_end > *pos {
                return Err(EsotereelError::ClipOverlap);
            }
        }

        self.clips.insert(new_pos, new_clip);
        Ok(())
    }

    pub fn get_clip_at(&self, pos: i64) -> Option<&Clip> {
        let (_, clip) = self.clips.range(..=pos).next_back()?;
        if pos < clip.position() + clip.duration {
            Some(clip)
        } else {
            None
        }
    }

    pub fn get_clip_by_id(&self, id: u64) -> Option<&Clip> {
        self.clips.values().find(|c| c.id == id)
    }

    pub fn remove_clip_by_id(&mut self, id: u64) -> Option<Clip> {
        let pos = self
            .clips
            .iter()
            .find_map(|(pos, c)| (c.id == id).then_some(*pos))?;
        self.clips.remove(&pos)
    }
}

// order のみで同一性を判定する
impl PartialEq for LayerModel {
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order
    }
}
impl Eq for LayerModel {}

impl Ord for LayerModel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order.cmp(&other.order)
    }
}
impl PartialOrd for LayerModel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
