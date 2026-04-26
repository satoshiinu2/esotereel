use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use rkyv::{
    Archive, CheckBytes, Deserialize, Serialize, bytecheck,
    with::{AsVec, Skip},
};

use crate::{
    project::clip::Clip,
    util::error::{EsotereelError, EsotereelResult},
};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Layer {
    pub index: usize,
    pub clips: ClipMap,
    pub name: String,
}

impl Layer {
    pub fn try_insert(&mut self, new_clip: Arc<Clip>) -> EsotereelResult<()> {
        let mut overlap = self.clips.range(new_clip.clone()..);

        // 次のクリップとの重なり
        if let Some(next) = overlap.next() {
            if new_clip.position + new_clip.duration > next.position {
                return Err(EsotereelError::ClipOverlap);
            }
        }

        // 前のクリップとの重なり
        let mut overlap_prev = self.clips.range(..new_clip.clone());
        if let Some(prev) = overlap_prev.next_back() {
            if prev.position + prev.duration > new_clip.position {
                return Err(EsotereelError::ClipOverlap);
            }
        }

        // 重なりがなければ挿入。
        self.clips.insert(new_clip);
        Ok(())
    }

    pub fn get_clip_at_frame(&self, frame: i64) -> Option<&Clip> {
        if let Some(clip) = self.clips.range(..=Clip::dummy_at(frame)).next_back() {
            if frame < clip.position + clip.duration {
                return Some(clip);
            }
        }
        None
    }
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]

pub struct ClipMap {
    #[with(AsVec)]
    pub tree: BTreeSet<Arc<Clip>>,

    #[with(Skip)]
    pub id_map: HashMap<u64, Arc<Clip>>,
}

impl Default for ClipMap {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipMap {
    pub fn new() -> Self {
        Self {
            tree: BTreeSet::new(),
            id_map: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.tree.len()
    }

    /// Rebuilds the `id_map` from the `clips` set.
    /// Necessary after deserialization as `id_map` is skipped by `rkyv`.
    pub fn rebuild_id_map(&mut self) {
        self.id_map = self
            .tree
            .iter()
            .map(|clip| (clip.id, clip.clone()))
            .collect();
    }

    pub fn range<T, R>(&self, range: R) -> std::collections::btree_set::Range<'_, Arc<Clip>>
    where
        T: Ord + ?Sized,
        Arc<Clip>: std::borrow::Borrow<T>,
        R: std::ops::RangeBounds<T>,
    {
        self.tree.range(range)
    }

    pub fn insert(&mut self, clip: Arc<Clip>) {
        self.tree.insert(clip.clone());
        self.id_map.insert(clip.id, clip.clone());
    }

    pub fn get_at(&self, pos: i64) -> Option<Arc<Clip>> {
        self.tree.get(&Clip::dummy_at(pos)).cloned()
    }

    pub fn get_by_id(&self, id: u64) -> Option<Arc<Clip>> {
        self.id_map.get(&id).cloned()
    }

    pub fn remove_at(&mut self, pos: i64) -> Option<Arc<Clip>> {
        if let Some(clip) = self.get_at(pos) {
            self.id_map.remove(&clip.id);
            self.tree.remove(&clip);
            Some(clip)
        } else {
            None
        }
    }

    pub fn remove_by_id(&mut self, id: u64) -> Option<Arc<Clip>> {
        if let Some(clip) = self.id_map.remove(&id) {
            self.tree.remove(&clip);
            Some(clip)
        } else {
            None
        }
    }

    pub fn hydrate_id_map(&mut self) {
        self.id_map = self.tree.iter().map(|c| (c.id, c.clone())).collect();
    }
}

impl<'a> IntoIterator for &'a ClipMap {
    type Item = &'a Arc<Clip>;
    type IntoIter = std::collections::btree_set::Iter<'a, Arc<Clip>>;

    fn into_iter(self) -> Self::IntoIter {
        self.tree.iter()
    }
}
