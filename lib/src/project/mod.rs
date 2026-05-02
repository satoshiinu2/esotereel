use std::{collections::HashMap, sync::Arc};

use crate::{
    project::{clip::Clip, clipdata::ClipData, timeline::Timeline},
    util::result::{EsotereelError, EsotereelResult},
};
use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

pub mod clip;
pub mod clipdata;
pub mod commands;
pub mod layer;
pub mod timeline;
pub mod util;

pub type ClipUpdateMap = HashMap<u32, Vec<Arc<Clip>>>;

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Project {
    timelines: [Timeline; 2],
}

impl Project {
    pub fn new() -> Self {
        Self {
            timelines: [Timeline::new(), Timeline::new()],
        }
    }

    pub fn get_timeline<'a>(&self, id: usize) -> EsotereelResult<&Timeline> {
        self.timelines
            .get(id)
            .ok_or(EsotereelError::InvalidTimeline)
    }
    pub fn get_timeline_mut<'a>(&mut self, id: usize) -> EsotereelResult<&mut Timeline> {
        self.timelines
            .get_mut(id)
            .ok_or(EsotereelError::InvalidTimeline)
    }
    pub fn get_timeline_count(&self) -> usize {
        self.timelines.len()
    }

    pub fn debug_add_clips(&mut self, timeline_idx: usize) {
        for i in 0..5 {
            let new_pl_clip = {
                let timeline = self.get_timeline_mut(timeline_idx).unwrap();
                timeline.new_clip(i * 100, 50, ClipData::Dummy)
            };

            self.get_timeline_mut(timeline_idx).unwrap().layers[i as usize]
                .try_insert(new_pl_clip)
                .unwrap();
        }
    }

    pub fn rebuild_id_map(&mut self) -> EsotereelResult<()> {
        for i in 0..self.get_timeline_count() {
            let timeline = self.get_timeline_mut(i)?;
            timeline
                .layers
                .iter_mut()
                .for_each(|l| l.clips.rebuild_id_map());
        }
        Ok(())
    }
}
