use std::sync::Arc;

use crate::project::{layer::Layer, layer_map::LayerMap};
use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Timeline {
    pub layers: LayerMap,
    pub fps: f64,
}

impl Timeline {
    pub fn new() -> Self {
        let mut s = Self {
            layers: LayerMap::new(),
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
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
