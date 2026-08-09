use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use crate::project::clip::Clip;
use crate::project::ids::ClipId;

#[derive(Debug, Default, Clone)]
pub struct ClipIndex {
    by_id: HashMap<ClipId, Clip>,
    by_position: BTreeMap<i64, ClipId>,
}

impl ClipIndex {
    pub fn new() -> Self {
        Self {
            by_id: Default::default(),
            by_position: Default::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn insert(&mut self, clip: Clip) {
        self.by_position.insert(clip.position, clip.id);
        self.by_id.insert(clip.id, clip);
    }

    pub fn get_by_id(&self, id: ClipId) -> Option<&Clip> {
        self.by_id.get(&id)
    }

    pub fn get_at(&self, pos: i64) -> Option<&Clip> {
        let (_, id) = self.by_position.range(..=pos).next_back()?;
        let clip = &self.by_id[id];
        (pos < clip.position + clip.duration).then_some(clip)
    }

    pub fn get_clips_in_range(&self, range: Range<i64>) -> Vec<&Clip> {
        let mut results = Vec::new();
        for (_, id) in self.by_position.range(..range.end).rev() {
            let clip = &self.by_id[id];
            if clip.position + clip.duration <= range.start {
                break;
            }
            results.push(clip);
        }
        results
    }

    /// 指定した範囲 (position .. position + duration) と重なるクリップがあるか走査
    pub fn overlaps(&self, position: i64, duration: i64, exclude: &[ClipId]) -> bool {
        let end = position + duration;

        self.by_position
            .range(..end)
            .map(|(_, id)| &self.by_id[id])
            .filter(|clip| !exclude.contains(&clip.id)) // 除外リストに含まれていないものだけを対象にする
            .any(|clip| clip.position + clip.duration > position)
    }

    pub fn remove_by_id(&mut self, id: ClipId) -> Option<Clip> {
        let clip = self.by_id.remove(&id)?;
        self.by_position.remove(&clip.position);
        Some(clip)
    }

    pub fn remove_at(&mut self, pos: i64) -> Option<Clip> {
        let id = self.get_at(pos)?.id;
        self.remove_by_id(id)
    }

    pub fn set_position(&mut self, id: ClipId, new_pos: i64) -> bool {
        let Some(mut clip) = self.remove_by_id(id) else {
            return false;
        };
        clip.position = new_pos;
        self.insert(clip);
        true
    }

    pub fn contains_id(&self, id: ClipId) -> bool {
        self.by_id.contains_key(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Clip> {
        self.by_position.values().map(move |id| &self.by_id[id])
    }

    /// 範囲の終端 (range.end) 未満から始まるクリップを対象にし、
    /// 開始位置 (range.start) より前で終わるクリップを filter で除外する
    pub fn range(&self, range: Range<i64>) -> impl Iterator<Item = &Clip> {
        self.by_position
            .range(..range.end)
            .map(move |(_, id)| &self.by_id[id])
            .filter(move |clip| clip.position + clip.duration > range.start)
    }
}
