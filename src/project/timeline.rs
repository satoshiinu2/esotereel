use crate::project::layer::Layer;

pub struct Timeline {
    pub layers: Vec<Layer>,
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
                    ],
                },
                Layer {
                    index: 1,
                    name: "Layer 1".to_string(),
                    clips: vec![crate::project::clip::Clip {
                        id: 2,
                        position: 20,
                        duration: 40,
                    }],
                },
                Layer {
                    index: 2,
                    name: "Layer 2".to_string(),
                    clips: vec![],
                },
                Layer {
                    index: 3,
                    name: "Layer 3".to_string(),
                    clips: vec![],
                },
            ],
        }
    }

    pub fn find_clip_by_id(&self, clip_id: usize) -> Option<(usize, usize)> {
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            for (clip_idx, clip) in layer.clips.iter().enumerate() {
                if clip.id == clip_id {
                    return Some((layer_idx, clip_idx));
                }
            }
        }
        None
    }
}
