use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck, with::Skip};
use std::{
    collections::{BTreeMap, HashMap},
    ops::Range,
    sync::Arc,
};

use crate::project::clip::Clip;

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[archive_attr(derive(CheckBytes))]

pub struct ClipMap {
    pub pos_tree: BTreeMap<i64, Arc<Clip>>, // key: position

    #[with(Skip)]
    pub id_map: HashMap<u64, Arc<Clip>>, // key: id
}

impl Default for ClipMap {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipMap {
    pub fn new() -> Self {
        Self {
            pos_tree: BTreeMap::new(),
            id_map: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.pos_tree.len()
    }

    // Necessary after deserialization
    pub fn rebuild_id_map(&mut self) {
        self.id_map = self
            .pos_tree
            .iter()
            .map(|(_, clip)| (clip.id, clip.clone()))
            .collect();
    }

    pub fn range<R>(&self, range: R) -> std::collections::btree_map::Range<'_, i64, Arc<Clip>>
    where
        R: std::ops::RangeBounds<i64>,
    {
        self.pos_tree.range(range)
    }

    pub fn insert(&mut self, clip: Arc<Clip>) {
        self.pos_tree.insert(clip.position(), clip.clone());
        self.id_map.insert(clip.id, clip.clone());
    }

    pub fn get_at(&self, pos: i64) -> Option<Arc<Clip>> {
        // 二分探索で「指定位置以前に始まる最後のクリップ」を取得
        let (_, found_clip) = self.range(..=pos).next_back()?;

        // 終端（position + duration）が指定位置を超えているかチェック
        if pos < found_clip.position() + found_clip.duration {
            Some(found_clip.clone())
        } else {
            None
        }
    }

    pub fn get_at_dbg(&self, pos: i64) -> Option<Arc<Clip>> {
        println!(
            "DEBUG: get_at({}) called. Map total count: {} {}",
            pos,
            self.pos_tree.len(),
            self.id_map.len()
        );

        // 全データの中身を強引に数件出す
        for (i, (k, v)) in self.pos_tree.iter().enumerate().take(5) {
            println!(
                "  [{}] Key: {}, ClipPos: {}, Dur: {}",
                i,
                k,
                v.position(),
                v.duration
            );
        }

        let result = self.pos_tree.range(..=pos).next_back();
        match result {
            Some((k, v)) => {
                println!(
                    "  Match Found! Key: {}, End: {}",
                    k,
                    v.position() + v.duration
                );
                if pos < v.position() + v.duration {
                    Some(v.clone())
                } else {
                    println!("  Rejected by duration check!");
                    None
                }
            }
            None => {
                println!("  No clip starts before or at {}", pos);
                None
            }
        }
    }

    pub fn get_clips_in_range(&self, range: Range<i64>) -> Vec<Arc<Clip>> {
        let mut results = Vec::new();

        // 1. end (範囲の終わり) 未満の中で、最も後ろにあるクリップから開始
        //    next_back() で後ろから順に辿る (O(log N) で位置特定)
        let mut iter = self.pos_tree.range(..range.end);

        while let Some((_, clip)) = iter.next_back() {
            // クリップの「終わり」が「検索範囲の始まり」より前なら、
            // これ以上前に遡っても絶対にヒットしないので即終了！
            if clip.position() + clip.duration <= range.start {
                break;
            }

            // 重なっているものを追加
            results.push(clip.clone());
        }

        results
    }

    pub fn get_by_id(&self, id: u64) -> Option<Arc<Clip>> {
        self.id_map.get(&id).cloned()
    }

    pub fn remove_at(&mut self, pos: i64) -> Option<Arc<Clip>> {
        if let Some(clip) = self.get_at(pos) {
            self.id_map.remove(&clip.id);
            self.pos_tree.remove(&clip.position());
            Some(clip)
        } else {
            None
        }
    }

    pub fn remove_by_id(&mut self, id: u64) -> Option<Arc<Clip>> {
        if let Some(clip) = self.id_map.remove(&id) {
            self.pos_tree.remove(&clip.position());
            Some(clip)
        } else {
            None
        }
    }

    // idが存在したらtrue
    pub fn set_position(&mut self, id: u64, new_pos: i64) -> bool {
        if let Some(clip_arc) = self.remove_by_id(id) {
            let mut updated_clip = (*clip_arc).clone();
            updated_clip.set_position(new_pos);

            self.insert(Arc::new(updated_clip));
            true
        } else {
            false
        }
    }

    pub fn contains_id(&self, clip_id: u64) -> bool {
        self.id_map.contains_key(&clip_id)
    }
}

impl<'a> IntoIterator for &'a ClipMap {
    type Item = (&'a i64, &'a Arc<Clip>);
    type IntoIter = std::collections::btree_map::Iter<'a, i64, Arc<Clip>>;

    fn into_iter(self) -> Self::IntoIter {
        self.pos_tree.iter()
    }
}
