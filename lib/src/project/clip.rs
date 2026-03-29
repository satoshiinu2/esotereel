use std::cmp::Ordering;

use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(Ord, PartialOrd, Eq, PartialEq))]
pub struct Clip {
    pub id: u64,
    pub position: i64,
    pub duration: i64,
}

impl Clip {
    pub fn dummy(pos: i64) -> Self {
        Self {
            id: 0,
            position: pos,
            duration: 0,
        }
    }
}

impl PartialEq for Clip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id // IDが同じなら同じクリップ
    }
}

impl Eq for Clip {}

impl Ord for Clip {
    fn cmp(&self, other: &Self) -> Ordering {
        // 1. まず開始位置で比較
        match self.position.cmp(&other.position) {
            Ordering::Equal => self.id.cmp(&other.id), // 同じ位置ならIDで順序を確定させる
            ord => ord,
        }
    }
}

impl PartialOrd for Clip {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
