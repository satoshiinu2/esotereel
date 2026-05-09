use std::sync::Arc;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::{
    project::{clip::Clip, clipmap::ClipMap},
    util::{
        result::{EsotereelError, EsotereelResult},
        slot_map::SlotMapKey,
    },
};
#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
#[archive_attr(derive(CheckBytes))]
pub struct Layer {
    pub order: u32,
    pub clips: ClipMap,
    pub name: String,
}

impl Layer {
    pub fn new(order: u32, name: String) -> Self {
        Self {
            order,
            clips: ClipMap::new(),
            name,
        }
    }

    pub fn try_insert(&mut self, new_clip: Arc<Clip>) -> EsotereelResult<()> {
        let mut next_range = self.clips.range(new_clip.position()..);

        // 次のクリップとの重なり
        if let Some((_, next)) = next_range.next() {
            if new_clip.position() + new_clip.duration > next.position() {
                return Err(EsotereelError::ClipOverlap);
            }
        }

        // 前のクリップとの重なり
        let mut prev_range = self.clips.range(..new_clip.position());
        if let Some((_, prev)) = prev_range.next_back() {
            if prev.position() + prev.duration > new_clip.position() {
                return Err(EsotereelError::ClipOverlap);
            }
        }

        // 重なりがなければ挿入。
        self.clips.insert(new_clip);
        Ok(())
    }
}
impl Eq for Layer {}

impl Ord for Layer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order.cmp(&other.order)
    }
}

impl PartialOrd for Layer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
#[repr(C)]
pub(crate) struct LayerMapKey {
    index: usize,
    generation: u32,
}

impl SlotMapKey for LayerMapKey {
    fn new(index: usize, generation: u32) -> Self {
        Self { index, generation }
    }

    fn index(&self) -> usize {
        self.index
    }

    fn generation(&self) -> u32 {
        self.generation
    }
}
