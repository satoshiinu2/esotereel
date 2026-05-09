use std::sync::Arc;

use crate::project::{clip::Clip, clipdata::ClipData, layer::Layer, layermap::LayerMap};
use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Timeline {
    pub layers: LayerMap,
    next_clip_id: u64,
    pub fps: f64,
}

impl Timeline {
    pub(crate) fn new() -> Self {
        let mut s = Self {
            layers: LayerMap::new(),
            next_clip_id: 0,
            fps: 60.0,
        };

        s.layers
            .insert(Arc::new(Layer::new(0, "Layer 0".to_string())));
        s.layers
            .insert(Arc::new(Layer::new(1, "Layer 1".to_string())));
        s.layers
            .insert(Arc::new(Layer::new(2, "Layer 2".to_string())));
        s.layers
            .insert(Arc::new(Layer::new(3, "Layer 3".to_string())));

        s
    }

    fn new_clip_id(&mut self) -> u64 {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        id
    }
    pub fn new_clip(&mut self, position: i64, duration: i64, clip_data: ClipData) -> Arc<Clip> {
        unsafe { Arc::new(Clip::new(self.new_clip_id(), position, duration, clip_data)) }
    }

    pub fn find_clip_by_id(&self, clip_id: u64) -> Option<(&Arc<Layer>, Arc<Clip>, usize)> {
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            if let Some(clip) = layer.clips.get_by_id(clip_id) {
                return Some((layer, clip, layer_idx));
            }
        }
        None
    }

    pub fn remove_clip_by_id(&mut self, clip_id: u64) -> Option<(&mut Arc<Layer>, Arc<Clip>, u32)> {//layer, clip, layer_handle
        for (layer_handle, layer) in self.layers.iter_mut().enumerate() {
            if let Some(clip) = Arc::make_mut(layer).clips.remove_by_id(clip_id) {
                return Some((layer, clip, layer_handle as u32));
            }
        }
        None
    }

    pub fn can_place_clip_at(
        &self,
        layer_idx: u32,
        position: i64,
        duration: i64,
        exclude_ids: &[u64],
    ) -> bool {
        if position < 0 {
            return false;
        }
        let Some(layer) = self.layers.get_by_layer_handle(layer_idx) else {
            return false;
        };

        for (_, clip) in &layer.clips {
            if exclude_ids.contains(&clip.id) {
                continue;
            }
            if position < clip.position() + clip.duration && position + duration > clip.position() {
                return false;
            }
        }
        true
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
