use std::collections::BTreeSet;

use crate::project::{clip::Clip, clipdata::ClipData, layer::Layer};
use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Timeline {
    pub layers: Vec<Layer>,
    next_clip_id: u64,
}

impl Timeline {
    pub(crate) fn new() -> Self {
        Self {
            layers: vec![
                Layer {
                    index: 0,
                    name: "Layer 0".to_string(),
                    clips: BTreeSet::new(),
                },
                Layer {
                    index: 1,
                    name: "Layer 1".to_string(),
                    clips: BTreeSet::new(),
                },
                Layer {
                    index: 2,
                    name: "Layer 2".to_string(),
                    clips: BTreeSet::new(),
                },
                Layer {
                    index: 3,
                    name: "Layer 3".to_string(),
                    clips: BTreeSet::new(),
                },
            ],
            next_clip_id: 0,
        }
    }

    fn new_clip_id(&mut self) -> u64 {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        id
    }
    pub fn new_clip(&mut self, position: i64, duration: i64, clip_data: ClipData) -> Clip {
        Clip {
            id: self.new_clip_id(),
            position,
            duration,
            clip_data,
        }
    }

    pub fn find_clip_by_id(&self, clip_id: u64) -> Option<(usize, usize, &Clip)> {
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            for (clip_idx, clip) in layer.clips.iter().enumerate() {
                if clip.id == clip_id {
                    return Some((layer_idx, clip_idx, clip));
                }
            }
        }
        None
    }

    pub fn remove_clip_by_id(&mut self, clip_id: u64) {
        for layer in self.layers.iter_mut() {
            layer.clips.retain(|c| c.id != clip_id);
        }
    }

    pub fn can_place_clip_at(
        &self,
        layer_idx: usize,
        position: i64,
        duration: i64,
        exclude_ids: &[u64],
    ) -> bool {
        if position < 0 {
            return false;
        }
        let Some(layer) = self.layers.get(layer_idx) else {
            return false;
        };

        for clip in &layer.clips {
            if exclude_ids.contains(&clip.id) {
                continue;
            }
            if position < clip.position + clip.duration && position + duration > clip.position {
                return false;
            }
        }
        true
    }
}
