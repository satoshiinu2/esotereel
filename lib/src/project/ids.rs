use rkyv::{Archive, CheckBytes, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize, bytecheck};
use serde::{Deserialize, Serialize};

pub type TimelineId = u64;
pub type LayerId = u64;
pub type ClipId = u64;

#[derive(Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Default, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct IdGenerator {
    next_timeline: TimelineId,
    next_layer: LayerId,
    next_clip: ClipId,
}

impl IdGenerator {
    pub fn next_timeline_id(&mut self) -> TimelineId {
        let id = self.next_timeline;
        self.next_timeline += 1;
        id
    }
    pub fn next_layer_id(&mut self) -> LayerId {
        let id = self.next_layer;
        self.next_layer += 1;
        id
    }
    pub fn next_clip_id(&mut self) -> ClipId {
        let id = self.next_clip;
        self.next_clip += 1;
        id
    }

    /// ロード時、既存idと衝突しないようカウンタを引き上げる
    pub fn observe_timeline(&mut self, id: TimelineId) {
        self.next_timeline = self.next_timeline.max(id + 1);
    }
    pub fn observe_layer(&mut self, id: LayerId) {
        self.next_layer = self.next_layer.max(id + 1);
    }
    pub fn observe_clip(&mut self, id: ClipId) {
        self.next_clip = self.next_clip.max(id + 1);
    }
}
