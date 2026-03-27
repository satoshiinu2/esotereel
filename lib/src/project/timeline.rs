use crate::project::{clip::Clip, layer::Layer};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct Timeline {
    pub layers: Vec<Layer>,
    pub playhead: i64,
}

impl Timeline {
    pub(crate) fn new() -> Self {
        Self {
            layers: vec![
                Layer {
                    index: 0,
                    name: "Layer 0".to_string(),
                    clips: vec![
                        crate::project::clip::Clip {
                            id: 0,
                            position: 10,
                            duration: 50,
                        },
                        crate::project::clip::Clip {
                            id: 1,
                            position: 70,
                            duration: 30,
                        },
                    ]
                    .into_iter()
                    .collect(),
                },
                Layer {
                    index: 1,
                    name: "Layer 1".to_string(),
                    clips: vec![crate::project::clip::Clip {
                        id: 2,
                        position: 20,
                        duration: 40,
                    }]
                    .into_iter()
                    .collect(),
                },
                Layer {
                    index: 2,
                    name: "Layer 2".to_string(),
                    clips: vec![].into_iter().collect(),
                },
                Layer {
                    index: 3,
                    name: "Layer 3".to_string(),
                    clips: vec![].into_iter().collect(),
                },
            ],
            playhead: 0,
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

    pub fn would_clip_overlap(
        &self,
        layer_idx: usize,
        position: u64,
        duration: u64,
        exclude_ids: &[u64],
    ) -> bool {
        let Some(layer) = self.layers.get(layer_idx) else {
            return true; // 範囲外と重なっている
        };

        for clip in &layer.clips {
            if exclude_ids.contains(&clip.id) {
                continue;
            }
            if position < clip.position + clip.duration && position + duration > clip.position {
                return true;
            }
        }
        false
    }
}
