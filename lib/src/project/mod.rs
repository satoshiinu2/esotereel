use std::{collections::HashMap, sync::Arc};

use crate::{
    project::{
        clip::Clip,
        clip_data::ClipData,
        clip_translate::{ClipTranslate, ClipTranslates},
        timeline::Timeline,
    },
    util::result::{EsotereelError, EsotereelResult},
};
use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

pub mod clip;
pub mod clip_data;
pub mod clip_map;
pub mod clip_translate;
pub mod commands;
pub mod layer;
pub mod layer_map;
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
        for i in 0..5u32 {
            let new_pl_clip = {
                let timeline = self.get_timeline_mut(timeline_idx).unwrap();
                let translates = ClipTranslates::Normal(ClipTranslate {
                    position: [100.0, 100.0, 0.0],
                    rotation: [0.0, 0.0, 0.0],
                    scale: [400.0, 300.0, 1.0],
                });

                timeline.new_clip(i as i64 * 100, 50, ClipData::Dummy, translates)
            };

            self.get_timeline_mut(timeline_idx)
                .unwrap()
                .layers
                .get_by_sorted_idx_mut(i)
                .map(|e| Arc::make_mut(e).try_insert(new_pl_clip).unwrap());
        }
    }

    pub fn rebuild_id_map(&mut self) -> EsotereelResult<()> {
        log::debug!("Rebuilding ID maps...");
        for i in 0..self.get_timeline_count() {
            let timeline = self.get_timeline_mut(i)?;

            timeline.layers.rebuild_id_map();

            timeline.layers.iter_mut().for_each(|l| {
                Arc::make_mut(l).clips.rebuild_id_map();
            });
        }

        Ok(())
    }
}
