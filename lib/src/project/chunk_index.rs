use std::collections::BTreeMap;
use std::ops::Range;

use crate::project::ids::{ClipId, LayerId};

pub const CHUNK_TICKS: i64 = 30_000;
type ChunkId = i64;

fn chunk_of(pos: i64) -> ChunkId {
    pos.div_euclid(CHUNK_TICKS)
}

/// Time -> (Layer, Clip) への純粋な検索インデックス。
/// データを所有しない。壊れても build() で再構築できるキャッシュ。
#[derive(Debug, Default, Clone)]
pub struct ChunkIndex {
    map: BTreeMap<ChunkId, Vec<(LayerId, ClipId)>>,
}

impl ChunkIndex {
    /// layer_clips: (LayerId, position, ClipId) のイテレータ。
    /// duration は見ない(start位置基準でチャンクに割り当てる)。
    pub fn build<'a>(entries: impl Iterator<Item = (LayerId, i64, ClipId)>) -> Self {
        let mut map: BTreeMap<ChunkId, Vec<(LayerId, ClipId)>> = BTreeMap::new();
        for (layer_id, pos, clip_id) in entries {
            map.entry(chunk_of(pos))
                .or_default()
                .push((layer_id, clip_id));
        }
        Self { map }
    }

    /// 範囲に触れる可能性のある (LayerId, ClipId) を返す。
    /// duration込みの厳密フィルタは呼び出し側(Timeline::query_range)で行う。
    pub fn candidates(&self, range: Range<i64>) -> impl Iterator<Item = (LayerId, ClipId)> + '_ {
        let start_c = chunk_of(range.start);
        let end_c = chunk_of(range.end.saturating_sub(1).max(range.start));
        self.map
            .range(start_c..=end_c)
            .flat_map(|(_, entries)| entries.iter().copied())
    }
}
