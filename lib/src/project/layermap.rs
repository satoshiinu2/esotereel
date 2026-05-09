use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck, with::Skip};
use std::{collections::BTreeMap, sync::Arc};

use std::slice::Iter;
use std::slice::IterMut;

use crate::project::clip::Clip;
use crate::project::layer::Layer;

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]

pub struct LayerMap {
    layers: Vec<Arc<Layer>>,

    #[with(Skip)]
    layer_order: BTreeMap<u32, Arc<Layer>>,
}

impl LayerMap {
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            layer_order: BTreeMap::new(),
        }
    }

    // Necessary after deserialization
    pub fn rebuild_id_map(&mut self) {
        // Vec の中身を元に BTreeMap を構築し直す
        self.layer_order = self
            .layers
            .iter()
            .map(|l| (l.order, Arc::clone(l)))
            .collect();
    }

    pub fn update_layer_clip(&mut self, layer_handle: u32, new_clip: Clip) {
        if let Some(layer_arc) = self.layers.get_mut(layer_handle as usize) {
            // ここで一回だけ make_mut
            let layer = Arc::make_mut(layer_arc);
            layer.clips.insert(Arc::new(new_clip));

            // その直後に自分で自分を同期する（外部に意識させない）
            let order = layer.order;
            let new_ptr = Arc::clone(layer_arc);
            self.layer_order.insert(order, new_ptr);
        }
    }

    pub fn insert(&mut self, layer: Arc<Layer>) {
        self.layers.push(Arc::clone(&layer));
        self.layer_order.insert(layer.order, Arc::clone(&layer));
    }

    // セッションでIDが同じ
    pub fn get_by_layer_handle(&self, index: u32) -> Option<Arc<Layer>> {
        self.layers.get(index as usize).cloned()
    }

    pub fn get_by_layer_handle_mut(&mut self, index: u32) -> Option<&mut Arc<Layer>> {
        self.layers.get_mut(index as usize)
    }

    pub fn get_by_sorted_idx(&self, id: u32) -> Option<Arc<Layer>> {
        self.layer_order.get(&id).cloned()
    }

    pub fn get_by_sorted_idx_mut(&mut self, id: u32) -> Option<&mut Arc<Layer>> {
        self.layer_order.get_mut(&id)
    }

    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn iter<'a>(&'a self) -> Iter<'a, Arc<Layer>> {
        self.layers.iter()
    }

    pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a, Arc<Layer>> {
        self.layers.iter_mut()
    }

    pub(crate) fn get_sorted_iter<'a>(
        &'a self,
    ) -> std::collections::btree_map::Values<'a, u32, Arc<Layer>> {
        self.layer_order.values()
    }
}

impl IntoIterator for LayerMap {
    type Item = Arc<Layer>;
    type IntoIter = std::vec::IntoIter<Arc<Layer>>;

    fn into_iter(self) -> Self::IntoIter {
        self.layers.into_iter()
    }
}

impl<'a> IntoIterator for &'a LayerMap {
    type Item = &'a Arc<Layer>;
    type IntoIter = Iter<'a, Arc<Layer>>;

    fn into_iter(self) -> Self::IntoIter {
        self.layers.iter()
    }
}

impl<'a> IntoIterator for &'a mut LayerMap {
    type Item = &'a mut Arc<Layer>;
    type IntoIter = IterMut<'a, Arc<Layer>>;

    fn into_iter(self) -> Self::IntoIter {
        self.layers.iter_mut()
    }
}
