use crate::project::layer::Layer;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Clone)]
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
            playhead: 0,
        }
    }

    pub fn find_clip_by_id(&self, clip_id: u32) -> Option<(usize, usize)> {
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layer(ptr: *const Timeline, l_idx: usize) -> *const Layer {
    if ptr.is_null() {
        return std::ptr::null();
    }

    let layers = unsafe { &(*ptr).layers };
    if l_idx < layers.len() {
        std::ptr::addr_of!(layers[l_idx])
    } else {
        std::ptr::null()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_layers_count(ptr: *const Timeline) -> usize {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).layers.len() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn timeline_get_playhead(ptr: *const Timeline) -> i64 {
    if ptr.is_null() {
        return 0;
    }

    unsafe { (*ptr).playhead }
}
