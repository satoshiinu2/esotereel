use rkyv::{CheckBytes, bytecheck};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::RwLock;

use crate::project::change::{ChangeSet, RemovedClipInfo};
use crate::project::chunk_index::ChunkIndex;
use crate::project::clip::{Clip, ClipData};
use crate::project::ids::{ClipId, IdGenerator, LayerId};
use crate::project::layer::{Layer, LayerMeta, LayerRemoveStrategy};
use crate::project::transform::ClipTranslates;
use crate::util::result::{EsotereelError, EsotereelResult};

/// Composite/Script/Mirrorの入れ子実行を無限ループさせないための上限。
pub const MAX_NESTED_DEPTH: u32 = 32;

#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Serialize, Deserialize, Debug)]
#[archive_attr(derive(CheckBytes))]
pub struct Timeline {
    pub id: u64,
    pub fps: f64,
    root_layers: Vec<LayerId>,
    layers: HashMap<LayerId, Layer>,

    /// Clip実体はここだけに存在する。Layer.clipsはidを参照するのみ。
    clips: HashMap<ClipId, Clip>,

    /// position検索用の遅延構築キャッシュ。保存対象外、壊れても再構築可能。
    #[with(rkyv::with::Skip)]
    #[serde(skip)]
    chunk_index: RwLock<Option<ChunkIndex>>,

    /// 未同期の差分
    #[with(rkyv::with::Skip)]
    #[serde(skip)]
    changes: ChangeSet,
}

impl Clone for Timeline {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            fps: self.fps,
            root_layers: self.root_layers.clone(),
            layers: self.layers.clone(),
            clips: self.clips.clone(),
            // キャッシュは持ち越さない。次回query_range時に再構築される。
            chunk_index: RwLock::new(None),
            changes: ChangeSet::default(),
        }
    }
}

impl Timeline {
    pub fn new(id: u64, fps: f64, ids: &mut IdGenerator) -> Self {
        let mut tl = Self {
            id,
            fps,
            root_layers: Vec::new(),
            layers: HashMap::new(),
            clips: HashMap::new(),
            chunk_index: RwLock::new(None),
            changes: ChangeSet::default(),
        };
        for i in 0..4u64 {
            let layer_id = ids.next_layer_id();
            let layer = Layer::new(layer_id, format!("Layer {}", i + 1));
            tl.root_layers.push(layer_id);
            tl.layers.insert(layer_id, layer);
        }
        tl
    }

    pub fn new_empty(id: u64, fps: f64) -> Self {
        Self {
            id,
            fps,
            root_layers: Vec::new(),
            layers: HashMap::new(),
            clips: HashMap::new(),
            chunk_index: RwLock::new(None),
            changes: ChangeSet::default(),
        }
    }

    fn invalidate_index(&self) {
        // Use write lock consistently for index invalidation
        let mut guard = self.chunk_index.write().unwrap();
        *guard = None;
    }

    // ---- Layer ----

    pub fn insert_layer(
        &mut self,
        layer: Layer,
        parent: Option<LayerId>,
        index: Option<usize>,
    ) -> EsotereelResult<()> {
        if self.layers.contains_key(&layer.id) {
            anyhow::bail!(EsotereelError::DuplicateLayerId(layer.id));
        }
        let id = layer.id;
        let mut layer = layer;
        layer.parent = parent;

        let siblings = match parent {
            Some(p) => {
                &mut self
                    .layers
                    .get_mut(&p)
                    .ok_or(EsotereelError::LayerNotFound)?
                    .children
            }
            None => &mut self.root_layers,
        };
        let idx = index.unwrap_or(siblings.len()).min(siblings.len());
        siblings.insert(idx, id);

        self.layers.insert(id, layer);

        self.changes.mark_layer_upserted(id);

        match parent {
            Some(p) => self.changes.mark_layer_upserted(p),
            None => self.changes.mark_root_layers_changed(),
        }
        Ok(())
    }

    pub fn remove_layer(
        &mut self,
        id: LayerId,
        strategy: LayerRemoveStrategy,
    ) -> EsotereelResult<Layer> {
        let layer = self
            .layers
            .remove(&id)
            .ok_or(EsotereelError::LayerNotFound)?;
        let grandparent = layer.parent;

        let removed_index = match grandparent {
            Some(p) => self
                .layers
                .get(&p)
                .and_then(|parent| parent.children.iter().position(|&c| c == id)),
            None => self.root_layers.iter().position(|&c| c == id),
        };

        // 親のchildrenから除去
        match grandparent {
            Some(p) => {
                if let Some(parent) = self.layers.get_mut(&p) {
                    parent.children.retain(|&c| c != id);
                }
                self.changes.mark_layer_upserted(p);
            }
            None => {
                self.root_layers.retain(|&c| c != id);
                self.changes.mark_root_layers_changed();
            }
        }

        // 子レイヤーも再帰削除(方針次第で変更)
        match strategy {
            LayerRemoveStrategy::Recursive => {
                for &child_id in layer.children.iter() {
                    // 戻り値は使わないが、再帰的にmark_layer_removedされる
                    let _ = self.remove_layer(child_id, LayerRemoveStrategy::Recursive)?;
                }
            }
            LayerRemoveStrategy::PromoteChildren => {
                let siblings = match grandparent {
                    Some(gp) => {
                        &mut self
                            .layers
                            .get_mut(&gp)
                            .ok_or(EsotereelError::LayerNotFound)?
                            .children
                    }
                    None => &mut self.root_layers,
                };

                let base = removed_index.unwrap_or(siblings.len()).min(siblings.len());
                for (offset, &child_id) in layer.children.iter().enumerate() {
                    siblings.insert(base + offset, child_id);
                }

                match grandparent {
                    Some(gp) => self.changes.mark_layer_upserted(gp),
                    None => self.changes.mark_root_layers_changed(),
                }

                for &child_id in layer.children.iter() {
                    if let Some(child) = self.layers.get_mut(&child_id) {
                        child.parent = grandparent;
                    }
                    self.changes.mark_layer_upserted(child_id);
                }
            }
        }

        self.changes.mark_layer_removed(id);
        Ok(layer)
    }

    /// クライアント側のローカル反映用。親のchildren書き換えはしない
    /// (親レイヤー自体もUpdateLayerで別途送られてくる前提)。
    pub fn remove_layer_local(&mut self, id: LayerId) -> Option<Layer> {
        self.layers.remove(&id)
    }

    pub fn move_layer(
        &mut self,
        id: LayerId,
        new_parent: Option<LayerId>,
        index: Option<usize>,
    ) -> EsotereelResult<()> {
        // 自分自身、または自分の子孫を新しい親にはできない(循環防止)
        if let Some(np) = new_parent {
            if np == id || self.is_descendant(np, id) {
                anyhow::bail!(EsotereelError::InvalidLayerMove);
            }
        }

        let old_parent = self
            .layers
            .get(&id)
            .ok_or(EsotereelError::LayerNotFound)?
            .parent;

        // 旧siblingsから除去
        match old_parent {
            Some(p) => {
                self.layers
                    .get_mut(&p)
                    .ok_or(EsotereelError::LayerNotFound)?
                    .children
                    .retain(|&c| c != id);
            }
            None => self.root_layers.retain(|&c| c != id),
        }

        // 新siblingsに挿入
        let siblings = match new_parent {
            Some(p) => {
                &mut self
                    .layers
                    .get_mut(&p)
                    .ok_or(EsotereelError::LayerNotFound)?
                    .children
            }
            None => &mut self.root_layers,
        };
        let idx = index.unwrap_or(siblings.len()).min(siblings.len());
        siblings.insert(idx, id);

        self.layers.get_mut(&id).unwrap().parent = new_parent;

        // 変更マーク：対象、旧親、新親のすべて
        self.changes.mark_layer_upserted(id);
        match old_parent {
            Some(p) => self.changes.mark_layer_upserted(p),
            None => self.changes.mark_root_layers_changed(),
        }
        match new_parent {
            Some(p) => self.changes.mark_layer_upserted(p),
            None => self.changes.mark_root_layers_changed(),
        }

        Ok(())
    }

    fn is_descendant(&self, candidate: LayerId, ancestor: LayerId) -> bool {
        let Some(layer) = self.layers.get(&ancestor) else {
            return false;
        };
        layer
            .children
            .iter()
            .any(|&c| c == candidate || self.is_descendant(candidate, c))
    }

    pub fn get_layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    pub fn get_layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    pub fn root_layers(&self) -> &[LayerId] {
        &self.root_layers
    }

    /// root_layers内でのindexを返す(旧orderの代替)。移動量の解決に使う。
    pub fn root_index_of(&self, layer_id: LayerId) -> Option<usize> {
        self.root_layers.iter().position(|&id| id == layer_id)
    }

    /// root_layers内のindexからLayerIdを返す。
    pub fn layer_id_at_root_index(&self, index: usize) -> Option<LayerId> {
        self.root_layers.get(index).copied()
    }

    pub fn set_root_layers(&mut self, root_layers: Vec<LayerId>) {
        self.root_layers = root_layers;
    }

    pub fn apply_layer_meta(&mut self, meta: LayerMeta) {
        let id = meta.id;
        match self.layers.get_mut(&id) {
            Some(existing) => {
                existing.name = meta.name;
                existing.enabled = meta.enabled;
                existing.parent = meta.parent;
                existing.children = meta.children;
                existing.folder = meta.folder;
                // clipsフィールドはLayerMetaに含まれないので保持する
            }
            None => {
                self.layers.insert(
                    id,
                    Layer {
                        id,
                        name: meta.name,
                        enabled: meta.enabled,
                        parent: meta.parent,
                        children: meta.children,
                        folder: meta.folder,
                        clips: BTreeMap::new(), // 新規なら空、clip自体は別Response経由で来る
                    },
                );
            }
        }
    }

    /// 並び替え。orderという数値の真実を持たないので重複チェック不要。
    pub fn reorder_child(
        &mut self,
        parent: Option<LayerId>,
        id: LayerId,
        new_index: usize,
    ) -> EsotereelResult<()> {
        let siblings = match parent {
            Some(p) => {
                &mut self
                    .layers
                    .get_mut(&p)
                    .ok_or(EsotereelError::LayerNotFound)?
                    .children
            }
            None => &mut self.root_layers,
        };
        let pos = siblings
            .iter()
            .position(|&x| x == id)
            .ok_or(EsotereelError::LayerNotFound)?;
        let id = siblings.remove(pos);
        siblings.insert(new_index.min(siblings.len()), id);
        Ok(())
    }

    /// root_layersをそのままの順で辿る。Folder(children持ち)は再帰的にフラット化する。
    pub fn iter_execution_order(&self) -> Vec<&Layer> {
        let mut out = Vec::new();
        self.flatten_into(&self.root_layers, &mut out);
        out
    }

    fn flatten_into<'a>(&'a self, ids: &[LayerId], out: &mut Vec<&'a Layer>) {
        for &id in ids {
            if let Some(layer) = self.layers.get(&id) {
                if layer.is_folder() {
                    self.flatten_into(&layer.children, out);
                } else {
                    out.push(layer);
                }
            }
        }
    }

    // ---- Clip ----

    pub fn new_clip_in(
        &mut self,
        layer_id: LayerId,
        ids: &mut IdGenerator,
        position: i64,
        duration: i64,
        data: ClipData,
        translates: ClipTranslates,
    ) -> EsotereelResult<ClipId> {
        // 重複チェック(既存 try_insert 相当)
        {
            let layer = self
                .layers
                .get(&layer_id)
                .ok_or(EsotereelError::LayerNotFound)?;
            let new_end = position + duration;
            for (&pos, &cid) in &layer.clips {
                let Some(existing) = self.clips.get(&cid) else {
                    continue;
                };
                let existing_end = pos + existing.duration;
                if position < existing_end && new_end > pos {
                    anyhow::bail!(EsotereelError::ClipOverlap);
                }
            }
        }

        let clip_id = ids.next_clip_id();
        let clip = Clip::new(clip_id, position, duration, data, translates);

        let layer = self
            .layers
            .get_mut(&layer_id)
            .ok_or(EsotereelError::LayerNotFound)?;

        layer.clips.insert(position, clip_id);
        self.clips.insert(clip_id, clip);

        self.touch_upsert(clip_id);

        Ok(clip_id)
    }

    pub fn remove_clip_by_id(&mut self, clip_id: ClipId) -> Option<(Clip, LayerId)> {
        let layer_id = self
            .layers
            .values()
            .find(|l| l.clips.values().any(|&id| id == clip_id))
            .map(|l| l.id)?;

        let clip = self.remove_clip_by_id_in(layer_id, clip_id)?;
        Some((clip, layer_id))
    }

    pub fn remove_clip_by_id_in(&mut self, layer_id: LayerId, clip_id: ClipId) -> Option<Clip> {
        let layer = self.layers.get_mut(&layer_id)?;

        layer.remove_clip(clip_id)?;
        let clip = self.clips.remove(&clip_id)?; // 削除できなかったらここで処理が終わる差分更新とかはされない

        self.touch_removed(&clip, layer_id);

        Some(clip)
    }

    /// 既にid確定済みのClipをそのままレイヤーに配置する。新規id発行はしない。
    /// undo/redoでの復元や、位置変更・レイヤー移動を伴う再配置に使う。
    /// 既に(別の場所に)存在するidなら、まず参照を除去してから配置し直す。
    pub fn place_clip(&mut self, layer_id: LayerId, clip: Clip) -> EsotereelResult<()> {
        let clip_id = clip.id;
        if self.clips.contains_key(&clip_id) {
            self.remove_clip_by_id(clip_id);
        }
        let layer = self
            .layers
            .get_mut(&layer_id)
            .ok_or(EsotereelError::LayerNotFound)?;

        layer.clips.insert(clip.position, clip_id);
        self.clips.insert(clip_id, clip);

        self.touch_upsert(clip_id);

        Ok(())
    }

    pub fn get_clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.get(&id)
    }

    pub fn iter_clips(&self) -> impl Iterator<Item = (&ClipId, &Clip)> {
        self.clips.iter()
    }

    /// Clipとその所属LayerIdを検索(非破壊)。C++側のfindClipById相当。
    pub fn find_clip_by_id(&self, clip_id: ClipId) -> Option<(&Clip, LayerId)> {
        let layer_id = self
            .layers
            .values()
            .find(|l| l.clips.values().any(|&id| id == clip_id))
            .map(|l| l.id)?;
        self.clips.get(&clip_id).map(|c| (c, layer_id))
    }

    /// 指定レイヤーの指定範囲にclipを置けるか(exclude_idsは自分自身などの除外用)。
    pub fn can_place_clip_at(
        &self,
        layer_id: LayerId,
        position: i64,
        duration: i64,
        exclude_ids: &[ClipId],
    ) -> bool {
        if position < 0 {
            return false;
        }
        let Some(layer) = self.layers.get(&layer_id) else {
            return false;
        };
        let new_end = position + duration;

        for (&pos, &cid) in &layer.clips {
            if exclude_ids.contains(&cid) {
                continue;
            }
            let Some(existing) = self.clips.get(&cid) else {
                continue;
            };
            let existing_end = pos + existing.duration;
            if position < existing_end && new_end > pos {
                return false;
            }
        }
        true
    }

    /// 指定レイヤーの、その時刻に実際に存在する(duration込みで判定した)Clipを取得。
    /// layer.get_clip_id_at は開始位置しか見ていないため、範囲判定はここで行う。
    pub fn get_clip_at(&self, layer_id: LayerId, pos: i64) -> Option<&Clip> {
        let layer = self.layers.get(&layer_id)?;
        let clip_id = layer.get_clip_id_at(pos)?;
        let clip = self.clips.get(&clip_id)?;
        (pos < clip.position + clip.duration).then_some(clip)
    }

    pub fn iter_layers(&self) -> impl Iterator<Item = (&LayerId, &Layer)> {
        self.layers.iter()
    }

    pub fn iter_layers_mut(&mut self) -> impl Iterator<Item = (&LayerId, &mut Layer)> {
        self.layers.iter_mut()
    }

    pub fn get_clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        // clip内容の書き換えはchunk_indexに影響しない(position/durationを
        // 変える場合は move_clip 経由にすること)
        self.clips.get_mut(&id)
    }

    /// クリップのposition変更。layer.clipsとchunk_indexの整合を保つ唯一の経路。
    pub fn move_clip(&mut self, clip_id: ClipId, new_position: i64) -> EsotereelResult<()> {
        let layer_id = self
            .layers
            .values()
            .find(|l| l.clips.values().any(|&id| id == clip_id))
            .map(|l| l.id)
            .ok_or(EsotereelError::LayerNotFound)?;

        let layer = self.layers.get_mut(&layer_id).unwrap();
        layer
            .remove_clip(clip_id)
            .ok_or(EsotereelError::LayerNotFound)?;
        layer.clips.insert(new_position, clip_id);

        if let Some(clip) = self.clips.get_mut(&clip_id) {
            clip.set_position(new_position);
        }

        self.touch_upsert(clip_id);

        Ok(())
    }

    /// 範囲検索。chunk_indexが無ければ遅延構築する。
    pub fn query_range(&self, range: Range<i64>) -> Vec<(&Layer, &Clip)> {
        // Single lock strategy: use write lock to handle both check and build
        // This avoids read-write lock upgrade deadlock
        let mut idx_guard = self.chunk_index.write().unwrap();

        if idx_guard.is_none() {
            let entries = self
                .layers
                .values()
                .flat_map(|l| l.clips.iter().map(move |(&pos, &cid)| (l.id, pos, cid)));
            *idx_guard = Some(ChunkIndex::build(entries));
        }

        let idx = idx_guard.as_ref().unwrap();

        idx.candidates(range.clone())
            .filter_map(|(layer_id, clip_id)| {
                let layer = self.layers.get(&layer_id)?;
                let clip = self.clips.get(&clip_id)?;
                let overlaps =
                    clip.position < range.end && clip.position + clip.duration > range.start;
                overlaps.then_some((layer, clip))
            })
            .collect()
    }

    pub(crate) fn touch_upsert(&mut self, id: ClipId) {
        self.invalidate_index();
        self.changes.clips_upserted.retain(|_| true); // no-op placeholder, see below
        self.changes.mark_clip_upserted(id);
    }

    pub(crate) fn touch_removed(&mut self, clip: &Clip, layer_id: LayerId) {
        self.invalidate_index();
        self.changes.mark_clip_removed(
            clip.id,
            RemovedClipInfo {
                layer_id,
                position: clip.position,
                duration: clip.duration,
            },
        );
    }

    pub fn drain_changes(&mut self) -> ChangeSet {
        std::mem::take(&mut self.changes)
    }

    // ---- Composite/Mirror/Script共通のネスト実行 ----

    /// このClipが参照する下位TimelineIdを返す(Composite/Area2D/Area3D/生成済みScript共通)。
    pub fn nested_timeline_id_of(&self, clip_id: ClipId) -> Option<u64> {
        self.clips
            .get(&clip_id)
            .and_then(|c| c.data.nested_timeline_id())
    }

    // ---- Independent化(deep clone) ----

    pub fn deep_clone(&self, ids: &mut IdGenerator, new_id: u64) -> Self {
        let mut cloned = self.clone();
        cloned.id = new_id;
        ids.observe_timeline(new_id);
        for &lid in cloned.layers.keys() {
            ids.observe_layer(lid);
        }
        for &cid in cloned.clips.keys() {
            ids.observe_clip(cid);
        }
        cloned
    }

    pub fn layers_meta(&self) -> Vec<LayerMeta> {
        self.layers.values().map(LayerMeta::from).collect()
    }

    // ---- クライアント側: ネットワーク経由の構築/マージ ----

    /// ProjectMeta受信時、構造だけからTimelineの骨格を作る(Clip無し)。
    pub fn from_meta(meta: &TimelineMeta) -> Self {
        let layers = meta
            .layers
            .iter()
            .map(|lm| {
                (
                    lm.id,
                    Layer {
                        id: lm.id,
                        name: lm.name.clone(),
                        enabled: lm.enabled,
                        parent: lm.parent,
                        children: lm.children.clone(),
                        folder: lm.folder,
                        clips: std::collections::BTreeMap::new(),
                    },
                )
            })
            .collect();

        Self {
            id: meta.id,
            fps: meta.fps,
            root_layers: meta.root_layers.clone(),
            layers,
            clips: HashMap::new(),
            chunk_index: RwLock::new(None),
            changes: ChangeSet::default(),
        }
    }

    /// FetchClipsInRangeの結果をマージ。サーバーが確定させたClipをそのまま挿入するので
    /// IdGeneratorは触らない(new_clip_inとは別経路)。
    pub fn merge_fetched_clips(&mut self, entries: Vec<(LayerId, Clip)>) {
        for (layer_id, clip) in entries {
            if let Some(layer) = self.layers.get_mut(&layer_id) {
                layer.clips.insert(clip.position, clip.id);
            }
            self.clips.insert(clip.id, clip);
        }
        self.invalidate_index();
    }

    /// ClipUpdates(差分同期)用: サーバー確定済みClipをupsertする。
    pub fn upsert_clip_from_network(&mut self, layer_id: LayerId, clip: Clip) {
        // 1. レイヤー跨ぎ移動に対応するため、全レイヤーからこのClipIdの古い参照を消去する
        for layer in self.layers.values_mut() {
            layer.clips.retain(|_, &mut cid| cid != clip.id);
        }

        // 2. 移動先のレイヤーへ配置する
        if let Some(layer) = self.layers.get_mut(&layer_id) {
            layer.clips.insert(clip.position, clip.id);
        }

        // 3. Clip実体を更新・保持する
        self.clips.insert(clip.id, clip);
        self.invalidate_index();
    }
}

/// ProjectAll用の軽量版。Clip本体を含まない。
#[derive(rkyv::Archive, rkyv::Serialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct TimelineMeta {
    pub id: u64,
    pub fps: f64,
    pub root_layers: Vec<LayerId>,
    pub layers: Vec<LayerMeta>,
}

impl From<&Timeline> for TimelineMeta {
    fn from(tl: &Timeline) -> Self {
        Self {
            id: tl.id,
            fps: tl.fps,
            root_layers: tl.root_layers.clone(),
            layers: tl.layers_meta(),
        }
    }
}
