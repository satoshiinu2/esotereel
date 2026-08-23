use std::collections::{BTreeMap, HashMap};

use crate::project::clip::ClipData;
use crate::project::ids::{ClipId, IdGenerator, LayerId};
use crate::project::model::layer::LayerModel;
use crate::project::model::timeline::TimelineModel;
use crate::project::runtime::clip_index::ClipIndex;
use crate::project::transform::ClipTranslates;
use crate::project::{Clip, ClipUpdateMap};
use crate::util::result::{EsotereelError, EsotereelResult};

#[derive(Debug, Clone)]
pub struct Layer {
    pub id: LayerId,
    pub order: u32,
    pub name: String,
    pub clips: ClipIndex,
}

impl Layer {
    pub fn new(id: u64, order: u32, name: String) -> Self {
        Self {
            id,
            order,
            name,
            clips: ClipIndex::new(),
        }
    }
}

#[derive(Debug)]
pub struct Timeline {
    pub id: u64,
    pub fps: f64,
    layer_order: BTreeMap<u32, LayerId>, // orderの一意性はここが唯一の真実
    layers: HashMap<LayerId, Layer>,
}

impl Timeline {
    pub fn new(id: u64, fps: f64) -> Self {
        Self {
            id,
            fps,
            layer_order: BTreeMap::new(),
            layers: HashMap::new(),
        }
    }

    pub fn insert_layer(&mut self, layer: Layer) -> EsotereelResult<()> {
        if self.layer_order.contains_key(&layer.order) {
            return Err(anyhow::Error::from(EsotereelError::DuplicateLayerOrder));
        }
        self.layer_order.insert(layer.order, layer.id);
        self.layers.insert(layer.id, layer);
        Ok(())
    }

    /// レイヤーへの変更はすべてここを通す。同期ロジックが一箇所にしか存在しない。
    pub fn modify_layer<F>(&mut self, id: LayerId, f: F) -> EsotereelResult<()>
    where
        F: FnOnce(&mut Layer),
    {
        let layer = self
            .layers
            .get_mut(&id)
            .ok_or_else(|| anyhow::Error::from(EsotereelError::LayerNotFound))?;
        let old_order = layer.order;
        f(layer);
        let new_order = layer.order;

        if new_order != old_order {
            if self.layer_order.contains_key(&new_order) {
                layer.order = old_order; // ロールバック
                return Err(anyhow::Error::from(EsotereelError::DuplicateLayerOrder));
            }
            self.layer_order.remove(&old_order);
            self.layer_order.insert(new_order, id);
        }
        Ok(())
    }

    pub fn new_clip_in<F: FnOnce(&Clip) -> ()>(
        &mut self,
        layer_id: LayerId,
        ids: &mut IdGenerator,
        position: i64,
        duration: i64,
        data: ClipData,
        translates: ClipTranslates,
        on_add: Option<F>,
    ) -> EsotereelResult<ClipId> {
        let clip_id = ids.next_clip_id();
        let clip = Clip {
            id: clip_id,
            position,
            duration,
            data,
            translates,
        };

        on_add.map(|f| f(&clip));

        self.modify_layer(layer_id, |layer| layer.clips.insert(clip))?;
        Ok(clip_id)
    }

    pub fn remove_clip_by_id(&mut self, clip_id: ClipId) -> Option<(Clip, LayerId)> {
        let layer_id = self
            .layers
            .values()
            .find(|l| l.clips.contains_id(clip_id))
            .map(|l| l.id)?;

        let mut removed = None;
        let _ = self.modify_layer(layer_id, |layer| {
            removed = layer.clips.remove_by_id(clip_id);
        });
        removed.map(|c| (c, layer_id))
    }

    pub fn find_clip_by_id(&self, clip_id: ClipId) -> Option<(&Layer, &Clip)> {
        self.layers
            .values()
            .find_map(|l| l.clips.get_by_id(clip_id).map(|c| (l, c)))
    }

    pub fn get_layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    pub fn get_layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    pub fn get_layer_id_by_order(&self, order: u32) -> Option<LayerId> {
        self.layer_order.get(&order).copied()
    }

    pub fn iter_layers(&self) -> impl Iterator<Item = (&LayerId, &Layer)> {
        self.layers.iter()
    }

    pub fn iter_layers_mut(&mut self) -> impl Iterator<Item = (&LayerId, &mut Layer)> {
        self.layers.iter_mut()
    }

    /// order順で走査(セーブ・表示用)
    pub fn iter_sorted(&self) -> impl Iterator<Item = &Layer> {
        self.layer_order.values().map(move |id| &self.layers[id])
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn can_place_clip_at(
        &self,
        layer_id: LayerId,
        position: i64,
        duration: i64,
        exclude: &[ClipId],
    ) -> bool {
        if position < 0 {
            return false;
        }
        match self.get_layer(layer_id) {
            Some(l) => !l.clips.overlaps(position, duration, exclude),
            None => false,
        }
    }

    // ---- ドメインモデルとの相互変換 ----

    pub fn to_model(&self) -> TimelineModel {
        let mut tl = TimelineModel::new(self.id, self.fps);
        tl.layers.clear();

        for layer_runtime in self.iter_sorted() {
            let mut layer = LayerModel::new(
                layer_runtime.id,
                layer_runtime.order,
                layer_runtime.name.clone(),
            );
            for clip in layer_runtime.clips.iter() {
                layer.clips.insert(clip.position, clip.clone());
            }
            tl.layers.insert(layer_runtime.order, layer);
        }

        tl
    }

    pub fn from_model(model: TimelineModel, ids: &mut IdGenerator) -> Self {
        ids.observe_timeline(model.id);
        let mut tl = Self::new(model.id, model.fps);

        for layer in model.layers.values() {
            ids.observe_layer(layer.id);
            let mut clips = ClipIndex::default();
            for clip in layer.clips.values() {
                ids.observe_clip(clip.id);
                clips.insert(clip.clone());
            }
            let layer_runtime = Layer {
                id: layer.id,
                order: layer.order,
                name: layer.name.clone(),
                clips,
            };
            // セーブデータ由来はorder重複しない前提。万一あれば無視せず伝える。
            let _ = tl.insert_layer(layer_runtime);
        }
        tl
    }
}
