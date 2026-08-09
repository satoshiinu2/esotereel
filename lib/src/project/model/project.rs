use rkyv::{CheckBytes, bytecheck};
use std::collections::BTreeMap;

use crate::project::{ids::IdGenerator, model::TimelineModel};

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
pub struct ProjectModel {
    pub timelines: BTreeMap<u64, TimelineModel>, // key: timeline id
    id_generator: IdGenerator,
}

impl Default for ProjectModel {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectModel {
    pub fn new() -> Self {
        Self {
            timelines: BTreeMap::new(),
            id_generator: IdGenerator::default(),
        }
    }

    /// 新しいタイムラインを作成
    pub fn new_timeline(&mut self, fps: f64) -> u64 {
        let id = self.id_generator.next_timeline_id();
        let timeline = TimelineModel::new(id, fps);
        self.timelines.insert(id, timeline);
        id
    }

    /// タイムラインを取得
    pub fn get_timeline(&self, id: u64) -> Option<&TimelineModel> {
        self.timelines.get(&id)
    }

    /// タイムラインを取得（可変）
    pub fn get_timeline_mut(&mut self, id: u64) -> Option<&mut TimelineModel> {
        self.timelines.get_mut(&id)
    }

    /// タイムラインを削除
    pub fn remove_timeline(&mut self, id: u64) -> Option<TimelineModel> {
        self.timelines.remove(&id)
    }

    /// タイムラインIDを観察して次のIDを調整（ロード時）
    pub fn observe_timeline_id(&mut self, id: u64) {
        self.id_generator.observe_timeline(id);
    }

    /// IDジェネレータへの参照（必要な場合）
    pub fn id_generator(&self) -> &IdGenerator {
        &self.id_generator
    }

    /// IDジェネレータへの可変参照（必要な場合）
    pub fn id_generator_mut(&mut self) -> &mut IdGenerator {
        &mut self.id_generator
    }

    /// rkyvデシリアライズ後にIDを再構築
    pub fn rebuild_id_map(&mut self) {
        for timeline in self.timelines.values_mut() {
            timeline.rebuild_next_clip_id();
        }
    }
}
