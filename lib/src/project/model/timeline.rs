use rkyv::{CheckBytes, bytecheck};
use std::collections::BTreeMap;

use crate::project::Clip;
use crate::project::clip::ClipData;
use crate::project::model::layer::LayerModel;
use crate::project::transform::ClipTranslates;
use crate::util::result::EsotereelError;

#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct TimelineModel {
    pub id: u64,
    pub fps: f64,
    pub layers: BTreeMap<u32, LayerModel>, // key: order

    #[with(rkyv::with::Skip)]
    next_clip_id: u64,
}

impl TimelineModel {
    pub fn new(id: u64, fps: f64) -> Self {
        let mut timeline = Self {
            id,
            fps,
            layers: BTreeMap::new(),
            next_clip_id: 0,
        };

        // 初期4レイヤーを作成
        for i in 0..4 {
            let layer = LayerModel::new(i as u64, i, format!("Layer {}", i));
            timeline.layers.insert(i, layer);
        }

        timeline
    }

    /// クリップを生成してレイヤーに追加（ID発行を一元化）
    pub fn new_clip_in(
        &mut self,
        layer_order: u32,
        position: i64,
        duration: i64,
        clip_data: ClipData,
        translates: ClipTranslates,
    ) -> Result<u64, EsotereelError> {
        let clip_id = self.next_clip_id;
        self.next_clip_id += 1;

        let clip = Clip::new(clip_id, position, duration, clip_data, translates);

        let layer = self
            .layers
            .get_mut(&layer_order)
            .ok_or(EsotereelError::LayerNotFound)?;

        layer.try_insert(clip)?;
        Ok(clip_id)
    }

    /// レイヤーを追加（order重複チェック）
    pub fn insert_layer(&mut self, layer: LayerModel) -> Result<(), EsotereelError> {
        if self.layers.contains_key(&layer.order) {
            return Err(EsotereelError::DuplicateLayerOrder);
        }
        self.layers.insert(layer.order, layer);
        Ok(())
    }

    /// レイヤーをorderで取得
    pub fn get_layer(&self, order: u32) -> Option<&LayerModel> {
        self.layers.get(&order)
    }

    /// レイヤーをorderで取得（可変）
    pub fn get_layer_mut(&mut self, order: u32) -> Option<&mut LayerModel> {
        self.layers.get_mut(&order)
    }

    /// order変更（重複チェック）
    pub fn modify_layer_order(
        &mut self,
        old_order: u32,
        new_order: u32,
    ) -> Result<(), EsotereelError> {
        if old_order == new_order {
            return Ok(());
        }

        if self.layers.contains_key(&new_order) {
            return Err(EsotereelError::DuplicateLayerOrder);
        }

        if let Some(mut layer) = self.layers.remove(&old_order) {
            layer.order = new_order;
            self.layers.insert(new_order, layer);
            Ok(())
        } else {
            Err(EsotereelError::LayerNotFound)
        }
    }

    /// 次に発行するclip idを復元（ロード時）
    pub fn restore_next_clip_id(&mut self, next_id: u64) {
        self.next_clip_id = next_id;
    }

    /// クリップIDを観察して次のIDを調整（ロード時）
    pub fn observe_clip_id(&mut self, clip_id: u64) {
        self.next_clip_id = self.next_clip_id.max(clip_id + 1);
    }

    /// rkyvデシリアライズ後に次のclip idを再構築
    pub fn rebuild_next_clip_id(&mut self) {
        let max_id = self
            .layers
            .values()
            .flat_map(|layer| layer.clips.values())
            .map(|clip| clip.id)
            .max()
            .unwrap_or(0);
        self.next_clip_id = max_id + 1;
    }
}
