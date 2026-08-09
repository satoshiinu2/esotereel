use std::collections::BTreeMap;

use crate::project::clip::ClipData;
use crate::project::ids::{ClipId, IdGenerator, LayerId, TimelineId};
use crate::project::model::{ProjectModel, TimelineModel};
use crate::project::runtime::timeline::Timeline;
use crate::project::transform::ClipTranslates;
use crate::project::{Clip, ClipUpdateMap};
use crate::util::result::{EsotereelError, EsotereelResult};

pub mod clip_index;
pub mod timeline;

#[derive(Debug, Default)]
pub struct Project {
    timelines: BTreeMap<TimelineId, Timeline>,
    ids: IdGenerator,
}

impl Project {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_timeline(&mut self, fps: f64) -> TimelineId {
        let id = self.ids.next_timeline_id();
        self.timelines.insert(id, Timeline::new(id, fps));
        id
    }

    pub fn timeline(&self, id: TimelineId) -> Option<&Timeline> {
        self.timelines.get(&id)
    }

    pub fn timeline_mut(&mut self, id: TimelineId) -> Option<&mut Timeline> {
        self.timelines.get_mut(&id)
    }

    pub fn timeline_count(&self) -> usize {
        self.timelines.len()
    }

    pub fn id_generator_mut(&mut self) -> &mut IdGenerator {
        &mut self.ids
    }

    /// 常に整合しているため、実質no-op。呼び出し元互換のために残す。
    pub fn rebuild_id_map(&mut self) -> EsotereelResult<()> {
        Ok(())
    }
    pub fn new_clip_in_timeline<F: FnOnce(&Clip) -> ()>(
        &mut self,
        timeline_id: TimelineId,
        layer_key: LayerId,
        position: i64,
        duration: i64,
        clip_data: ClipData,
        translates: ClipTranslates,
        on_add: Option<F>,
    ) -> EsotereelResult<ClipId> {
        let timeline = self
            .timelines
            .get_mut(&timeline_id)
            .ok_or(EsotereelError::InvalidTimeline)?;

        timeline.new_clip_in(
            layer_key,
            &mut self.ids,
            position,
            duration,
            clip_data,
            translates,
            on_add,
        )
    }

    // ---- ドメインモデルとの相互変換 ----

    pub fn to_model(&self) -> ProjectModel {
        let mut pj = ProjectModel::new();
        pj.timelines.clear();

        for tl in &self.timelines {
            let tl_model = tl.1.to_model();

            pj.timelines.insert(*tl.0, tl_model);
        }

        pj
    }

    pub fn from_model(model: ProjectModel) -> Self {
        let mut pj = Self::new();

        for tl_model in model.timelines {
            let tl = Timeline::from_model(tl_model.1, &mut pj.ids);

            pj.timelines.insert(tl_model.0, tl);
        }
        pj
    }
}
