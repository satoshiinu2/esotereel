use std::{collections::HashMap, sync::Arc};

use crate::{
    project::{
        clip::Clip,
        clip_data::ClipData,
        clip_translate::{ClipTranslate, ClipTranslates},
        timeline::Timeline,
    },
    util::{
        result::{EsotereelError, EsotereelResult},
        slot_map::{SlotMap, SlotMapKey},
    },
};
use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

pub mod camera;
pub mod clip;
pub mod clip_data;
pub mod clip_map;
pub mod clip_translate;
pub mod commands;
pub mod layer;
pub mod layer_map;
pub mod timeline;
pub mod util;

pub type ClipUpdateMap = HashMap<SlotMapKey, Vec<Arc<Clip>>>; // layer clips

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Project {
    pub timelines: SlotMap<Timeline>,
}

impl Project {
    pub fn new() -> Self {
        Self {
            timelines: SlotMap::new(),
        }
    }

    pub fn debug_add_clips(&mut self, timeline_idx: usize) {
        let key = self.timelines.get_cureent_new_key(timeline_idx);

        for i in 0..5u32 {
            let new_pl_clip = {
                let timeline = self.timelines.get_mut(&key).unwrap();
                let translates = ClipTranslates::Normal(ClipTranslate {
                    position: [100.0, 100.0, 0.0],
                    rotation: [0.0, 0.0, 0.0],
                    scale: [400.0, 300.0, 1.0],
                });

                timeline
                    .layers
                    .new_clip(i as i64 * 100, 50, ClipData::Dummy, translates)
            };

            let layers = &mut self.timelines.get_mut(&key).unwrap().layers;
            let key = layers.get_cureent_new_key(i as usize);

            layers.modify_layer(&key, |l| l.clips.insert(new_pl_clip));
        }
    }

    pub fn rebuild_id_map(&mut self) -> EsotereelResult<()> {
        log::debug!("Rebuilding ID maps...");
        for i in 0..self.timelines.len() {
            let key = self.timelines.get_cureent_new_key(i);

            let timeline = self
                .timelines
                .get_mut(&key)
                .ok_or(EsotereelError::InvalidTimeline)?;

            timeline.layers.rebuild_id_map();

            timeline.layers.for_each_layer_mut(|l| {
                l.clips.rebuild_id_map();
            });
        }

        Ok(())
    }
}
