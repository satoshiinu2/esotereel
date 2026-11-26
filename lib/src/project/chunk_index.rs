use std::collections::BTreeMap;
use std::ops::Range;

use crate::project::ids::{ClipId, LayerId};

pub const CHUNK_TICKS: i64 = 30_000;
type ChunkId = i64;

fn chunk_of(pos: i64) -> ChunkId {
    pos.div_euclid(CHUNK_TICKS)
}

/// 1チャンク分の索引エントリです。
#[derive(Debug, Default, Clone)]
struct ChunkEntry {
    /// (LayerId, ClipId) の列。開始位置がこのチャンクに入るクリップだけ。
    clips: Vec<(LayerId, ClipId)>,
    /// このチャンク内のクリップの終了 tick の最大値。
    /// チャンクをまたぐ長いクリップを範囲検索で拾うために使います。
    max_end: i64,
}

/// Time -> (Layer, Clip) への純粋な検索インデックス。
/// データを所有しない。壊れても build() で再構築できるキャッシュ。
#[derive(Debug, Default, Clone)]
pub struct ChunkIndex {
    map: BTreeMap<ChunkId, ChunkEntry>,
}

impl ChunkIndex {
    /// layer_clips: (LayerId, position, duration, ClipId) のイテレータ。
    /// duration は max_end の計算に使い、所有は開始位置基準です。
    pub fn build<'a>(
        entries: impl Iterator<Item = (LayerId, i64, i64, ClipId)>,
    ) -> Self {
        let mut map: BTreeMap<ChunkId, ChunkEntry> = BTreeMap::new();
        for (layer_id, pos, duration, clip_id) in entries {
            let entry = map.entry(chunk_of(pos)).or_default();
            entry.clips.push((layer_id, clip_id));
            entry.max_end = entry.max_end.max(pos + duration);
        }
        Self { map }
    }

    /// 範囲に触れる可能性のある (LayerId, ClipId) を返す。
    /// duration込みの厳密フィルタは呼び出し側(Timeline::query_range)で行う。
    ///
    /// 開始が範囲より前のチャンクにある長いクリップも拾うため、範囲開始を
    /// 超える max_end を持つチャンクまで遡って走査します。
    pub fn candidates(&self, range: Range<i64>) -> impl Iterator<Item = (LayerId, ClipId)> + '_ {
        let end_c = chunk_of(range.end.saturating_sub(1).max(range.start));
        let mut start_c = chunk_of(range.start);
        while let Some((&prev, entry)) = self.map.range(..start_c).next_back() {
            if entry.max_end > range.start {
                start_c = prev;
            } else {
                break;
            }
        }
        self.map
            .range(start_c..=end_c)
            .flat_map(|(_, entry)| entry.clips.iter().copied())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/project/chunk_index_tests.rs"]
mod tests;
