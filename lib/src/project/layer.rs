use std::collections::BTreeSet;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::{
    project::clip::Clip,
    util::error::{EsotereelError, EsotereelResult},
};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(Ord, PartialOrd, Eq, PartialEq, CheckBytes))]
pub struct Layer {
    pub index: usize,
    pub clips: BTreeSet<Clip>,
    pub name: String,
}

impl Layer {
    pub fn try_insert(&mut self, new_clip: Clip) -> EsotereelResult<()> {
        // 1. 挿入したい位置の「直前」と「直後」だけをチェックする
        // position が new_clip.position 以上の最初の要素を取得
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

        // 重なりがなければ挿入。O(log N) で爆速
        self.clips.insert(new_clip);
        Ok(())
    }
}
