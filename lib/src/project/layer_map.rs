use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck, with::Skip};
use std::{collections::BTreeMap, sync::Arc};

use crate::project::clip::Clip;
use crate::project::clip_data::ClipData;
use crate::project::clip_translate::ClipTranslates;
use crate::project::layer::Layer;
use crate::util::slot_map::{Iter as SlotIter, SlotMap, SlotMapKey};

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]

pub struct LayerMap {
    layers: SlotMap<Arc<Layer>>,

    #[with(Skip)]
    layer_order: BTreeMap<u32, Arc<Layer>>,

    #[with(Skip)]
    next_clip_id: u64,
}

impl LayerMap {
    pub fn new() -> Self {
        Self {
            layers: SlotMap::new(),
            layer_order: BTreeMap::new(),
            next_clip_id: 0,
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

    pub fn update_layer_clip(&mut self, key: &SlotMapKey, new_clip: Clip) {
        if let Some(layer_arc) = self.layers.get_mut(key) {
            // ここで一回だけ make_mut
            let layer = Arc::make_mut(layer_arc);
            layer.clips.insert(Arc::new(new_clip));

            // その直後に自分で自分を同期する（外部に意識させない）
            let order = layer.order;
            let new_ptr = Arc::clone(layer_arc);
            self.layer_order.insert(order, new_ptr);
        }
    }

    pub fn get_cureent_new_key(&self, idx: usize) -> SlotMapKey {
        self.layers.get_cureent_new_key(idx)
    }

    fn new_clip_id(&mut self) -> u64 {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        id
    }

    pub fn new_clip(
        &mut self,
        position: i64,
        duration: i64,
        clip_data: ClipData,
        translates: ClipTranslates,
    ) -> Arc<Clip> {
        unsafe {
            Arc::new(Clip::new(
                self.new_clip_id(),
                position,
                duration,
                clip_data,
                translates,
            ))
        }
    }

    pub fn insert(&mut self, layer: Arc<Layer>) {
        self.layers.insert(Arc::clone(&layer));
        self.layer_order.insert(layer.order, Arc::clone(&layer));
    }

    pub fn remove_clip_by_id(&mut self, clip_id: u64) -> Option<(Arc<Clip>, SlotMapKey)> {
        // まず対象 layer を探す
        let target_key = self
            .layers
            .iter_with_key()
            .find_map(|(key, layer)| layer.clips.contains_id(clip_id).then_some(key))?;

        // 必要になってから mutable access
        let layer_arc = self.layers.get_mut(&target_key)?;

        let layer = Arc::make_mut(layer_arc);

        let clip = layer.clips.remove_by_id(clip_id)?;

        self.layer_order.insert(layer.order, Arc::clone(layer_arc));

        Some((clip, target_key))
    }

    pub fn for_each_layer_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Layer),
    {
        for layer_arc in self.layers.iter_mut() {
            let old_order = layer_arc.order;

            let layer = Arc::make_mut(layer_arc);

            f(layer);

            if old_order != layer.order {
                self.layer_order.remove(&old_order);
            }

            self.layer_order.insert(layer.order, Arc::clone(layer_arc));
        }
    }

    pub fn find_clip_by_id(&self, clip_id: u64) -> Option<(&Arc<Layer>, Arc<Clip>, SlotMapKey)> {
        for (layer_map_key, layer) in self.layers.iter_with_key() {
            if let Some(clip) = layer.clips.get_by_id(clip_id) {
                return Some((layer, clip, layer_map_key));
            }
        }
        None
    }

    pub fn find_orderd_clip_by_id(&self, clip_id: u64) -> Option<(&Arc<Layer>, Arc<Clip>, u32)> {
        for (layer_index, layer) in self.layer_order.iter() {
            if let Some(clip) = layer.clips.get_by_id(clip_id) {
                return Some((layer, clip, *layer_index));
            }
        }
        None
    }

    pub fn get_layer_map_key_by_order(&self, order: u32) -> Option<SlotMapKey> {
        self.layers
            .iter_with_key()
            .find_map(|(key, l)| (l.order == order).then_some(key))
    }

    pub fn can_place_clip_at(
        &self,
        layer_key: &SlotMapKey,
        position: i64,
        duration: i64,
        exclude_ids: &[u64],
    ) -> bool {
        if position < 0 {
            return false;
        }
        let Some(layer) = self.get_by_layer_map_key(&layer_key) else {
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

    // セッションでIDが同じ
    pub fn get_by_layer_map_key(&self, key: &SlotMapKey) -> Option<Arc<Layer>> {
        self.layers.get(key).cloned()
    }

    pub fn get_by_sorted_idx(&self, id: u32) -> Option<Arc<Layer>> {
        self.layer_order.get(&id).cloned()
    }

    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn iter(&self) -> SlotIter<'_, Arc<Layer>> {
        self.layers.iter()
    }

    #[must_use]
    pub fn iter_with_key(&self) -> impl Iterator<Item = (SlotMapKey, &Arc<Layer>)> {
        self.layers.iter_with_key()
    }

    #[must_use]
    pub fn entry(&self) -> SlotIter<'_, Arc<Layer>> {
        self.layers.iter()
    }

    #[must_use]
    pub fn get_sorted_iter<'a>(
        &'a self,
    ) -> std::collections::btree_map::Values<'a, u32, Arc<Layer>> {
        self.layer_order.values()
    }

    pub fn modify_layer<F>(&mut self, key: &SlotMapKey, f: F)
    where
        F: FnOnce(&mut Layer),
    {
        if let Some(layer_arc) = self.layers.get_mut(key) {
            let old_order = layer_arc.order;

            let layer = Arc::make_mut(layer_arc);

            f(layer);

            // 順番更新
            if old_order != layer.order {
                self.layer_order.remove(&old_order);
            }

            self.layer_order.insert(layer.order, Arc::clone(layer_arc));
        }
    }
}

impl<'a> IntoIterator for &'a LayerMap {
    type Item = &'a Arc<Layer>;
    type IntoIter = SlotIter<'a, Arc<Layer>>;

    fn into_iter(self) -> Self::IntoIter {
        self.layers.iter()
    }
}
