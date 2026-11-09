use rkyv::{CheckBytes, bytecheck};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::RwLock;

use crate::plugin::NamespacedID;
use crate::project::TimelineTick;
use crate::project::change::{ChangeSet, RemovedClipInfo};
use crate::project::chunk_index::ChunkIndex;
use crate::project::clip::Clip;
use crate::project::clip::ClipBindingValue;
use crate::project::ids::{ClipId, IdGenerator, LayerFolderId, LayerId};
use crate::project::layer::{Layer, LayerMeta, LayerRemoveStrategy};
use crate::project::layer_outline::{LayerFolder, LayerOutline, Meta, OutlineNode};
use crate::util::result::{EsotereelError, EsotereelResult};

/// Composite / Script / Mirror の入れ子実行を無限ループさせないための上限。
pub const MAX_NESTED_DEPTH: u32 = 32;

/// タイムラインの実体です。
///
/// レイヤー（`Layer`）、クリップ（`Clip`）、アウトライン（`LayerOutline`）を保持し、
/// 編集操作と変更セット（`ChangeSet`）の管理を行います。
///
/// - `clips` は Clip 実体の唯一の保持場所で、`Layer.clips` は位置 → `ClipId` の参照のみを保持します。
/// - `chunk_index` は位置検索用の遅延構築キャッシュで、保存対象外です。
/// - `changes` は未同期の差分で、ネットワーク同期時に `drain_changes` で取り出されます。
#[derive(
    rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde:: Deserialize, Debug,
)]
#[archive_attr(derive(CheckBytes))]
pub struct Timeline {
    pub id: u64,
    pub tps: f64,
    layers: HashMap<LayerId, Layer>,

    /// Clip 実体はここだけに存在する。`Layer.clips` は id を参照するのみ。
    clips: HashMap<ClipId, Clip>,

    pub outline: LayerOutline,

    /// 位置検索用の遅延構築キャッシュ。保存対象外、壊れても再構築可能。
    #[with(rkyv::with::Skip)]
    #[serde(skip)]
    chunk_index: RwLock<Option<ChunkIndex>>,

    /// 未同期の差分。
    #[with(rkyv::with::Skip)]
    #[serde(skip)]
    changes: ChangeSet,
}

impl Clone for Timeline {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tps: self.tps,
            layers: self.layers.clone(),
            outline: self.outline.clone(),
            clips: self.clips.clone(),
            // キャッシュは持ち越さない。次回query_range時に再構築される。
            chunk_index: RwLock::new(None),
            changes: ChangeSet::default(),
        }
    }
}

impl Timeline {
    /// 新しいタイムラインを作成し、デフォルトで 4 つのレイヤーを挿入します。
    pub fn new(id: u64, fps: f64, ids: &mut IdGenerator) -> Self {
        let mut tl = Self {
            id,
            tps: fps,
            layers: HashMap::new(),
            clips: HashMap::new(),
            chunk_index: RwLock::new(None),
            changes: ChangeSet::default(),
            outline: LayerOutline::default(),
        };

        for i in 0..4 {
            tl.insert_layer(ids, format!("Layer {}", i + 1), None, None);
        }
        tl
    }

    /// 空のタイムラインを作成します（レイヤーなし）。
    pub fn new_empty(id: u64, tps: f64) -> Self {
        Self {
            id,
            tps,
            layers: HashMap::new(),
            outline: LayerOutline::default(),
            clips: HashMap::new(),
            chunk_index: RwLock::new(None),
            changes: ChangeSet::default(),
        }
    }

    fn invalidate_index(&self) {
        // Use write lock consistently for index invalidation
        let Ok(mut guard) = self.chunk_index.write() else {
            log::warn!("timeline: chunk_index write lock poisoned");
            return;
        };
        *guard = None;
    }

    // Layer

    pub fn insert_layer(
        &mut self,
        ids: &mut IdGenerator,
        name: String,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
    ) -> LayerId {
        let id = ids.next_layer_id();
        self.layers.insert(id, Layer::new(id, name));
        self.changes.mark_layer_upserted(id);

        self.outline.insert_layer(id, parent, index);
        self.changes.mark_outline_children_changed(parent);

        id
    }

    pub fn insert_folder(
        &mut self,
        ids: &mut IdGenerator,
        name: String,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
    ) -> LayerFolderId {
        let id = ids.next_folder_id();
        self.outline.insert_folder(id, name, parent, index);
        self.changes.mark_outline_folder_upserted(id);
        self.changes.mark_outline_children_changed(parent);
        id
    }

    pub fn remove_layer(&mut self, id: LayerId) -> Option<Layer> {
        let layer = self.layers.remove(&id)?;
        if let Some(parent) = self.outline.parent_of(&OutlineNode::Layer(id)) {
            self.outline.remove_layer(id);
            self.changes
                .mark_outline_children_changed(parent.to_option());
        }
        self.changes.mark_layer_removed(id);
        Some(layer)
    }

    pub fn remove_folder(
        &mut self,
        id: LayerFolderId,
        strategy: LayerRemoveStrategy,
    ) -> EsotereelResult<()> {
        let parent = self.outline.parent_of(&OutlineNode::Folder(id));
        let leftover = self.outline.remove_folder(id, strategy);
        self.changes.mark_outline_folder_removed(id);
        if let Some(parent) = parent {
            self.changes
                .mark_outline_children_changed(parent.to_option());
        }

        // Recursiveのときだけ、中身をTimelineから再帰的に本当に削除する
        for node in leftover {
            match node {
                OutlineNode::Layer(lid) => {
                    self.remove_layer(lid);
                }
                OutlineNode::Folder(fid) => {
                    self.remove_folder(fid, LayerRemoveStrategy::Recursive)?;
                }
            }
        }
        Ok(())
    }

    /// クライアント側のローカル反映用。親の `children` 書き換えは行いません。
    ///
    /// 親レイヤー自体は `UpdateLayer` で別途送られてくる前提です。
    pub fn remove_layer_local(&mut self, id: LayerId) -> Option<Layer> {
        self.layers.remove(&id)
    }

    pub fn move_node(
        &mut self,
        node: OutlineNode,
        new_parent: Option<LayerFolderId>,
        index: Option<usize>,
    ) -> EsotereelResult<()> {
        let old_parent = self.outline.parent_of(&node).ok_or_else(|| {
            EsotereelError::LayerNotFound(match node {
                OutlineNode::Layer(id) => id,
                OutlineNode::Folder(id) => id as LayerId,
            })
        })?;
        self.outline.move_node(node, new_parent, index)?;

        self.changes
            .mark_outline_children_changed(old_parent.to_option());
        self.changes.mark_outline_children_changed(new_parent);
        Ok(())
    }

    pub fn layers_len(&self) -> usize {
        self.layers.len()
    }

    pub fn get_layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    pub fn get_layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    pub fn get_folder(&self, id: LayerFolderId) -> Option<&LayerFolder> {
        self.outline.get_folder(id)
    }
    pub fn get_folder_mut(&mut self, id: LayerFolderId) -> Option<&mut LayerFolder> {
        self.outline.get_folder_mut(id)
    }

    pub fn apply_layer_meta(&mut self, meta: LayerMeta) {
        let id = meta.id;
        match self.layers.get_mut(&id) {
            Some(existing) => {
                existing.name = meta.name;
                existing.enabled = meta.enabled;
            }
            None => {
                self.layers.insert(
                    id,
                    Layer {
                        id,
                        name: meta.name,
                        enabled: meta.enabled,
                        clips: BTreeMap::new(), // 新規なら空、clip自体は別Response経由で来る
                    },
                );
            }
        }
    }

    /// レンダリング・合成実行で使用する「実際の重ね合わせ順」でレイヤーを反復します。
    ///
    /// `iter_layers()` は `HashMap` 由来で順序不定ですが、こちらは常に決まった順序を返します。
    pub fn iter_execution_order(&self) -> impl Iterator<Item = &Layer> + '_ {
        self.outline
            .iter_execution_order()
            .filter_map(move |id| self.layers.get(&id))
    }

    // Clip

    pub fn new_clip_in(
        &mut self,
        layer_id: LayerId,
        ids: &mut IdGenerator,
        position: TimelineTick,
        duration: TimelineTick,
        kind_id: NamespacedID,
        properties: HashMap<NamespacedID, ClipBindingValue>,
    ) -> EsotereelResult<ClipId> {
        // 重複チェック(既存 try_insert 相当)
        {
            let layer = self
                .layers
                .get(&layer_id)
                .ok_or(EsotereelError::LayerNotFound(layer_id))?;
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
        let clip = Clip::new(clip_id, position, duration, kind_id, properties);

        let layer = self
            .layers
            .get_mut(&layer_id)
            .ok_or(EsotereelError::LayerNotFound(layer_id))?;

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

    /// 既に ID 確定済みの `Clip` をそのままレイヤーに配置します。新規 ID は発行しません。
    ///
    /// Undo/Redo での復元や、位置変更・レイヤー移動を伴う再配置に使用します。
    /// 既に別の場所に存在する同 ID の場合は、まず参照を除去してから配置し直します。
    pub fn place_clip(&mut self, layer_id: LayerId, clip: Clip) -> EsotereelResult<()> {
        let clip_id = clip.id;
        if self.clips.contains_key(&clip_id) {
            self.remove_clip_by_id(clip_id);
        }
        let layer = self
            .layers
            .get_mut(&layer_id)
            .ok_or(EsotereelError::LayerNotFound(layer_id))?;

        layer.clips.insert(clip.position, clip_id);
        self.clips.insert(clip_id, clip);

        self.touch_upsert(clip_id);

        Ok(())
    }

    pub fn get_clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.get(&id)
    }

    /// Clipとその所属LayerIdを検索
    pub fn get_clip_and_layer(&self, clip_id: ClipId) -> Option<(&Clip, LayerId)> {
        let clip = self.clips.get(&clip_id)?;

        let layer_id = self
            .layers
            .values()
            .find(|layer| layer.clips.get(&clip.position) == Some(&clip_id))
            .map(|layer| layer.id)?;

        Some((clip, layer_id))
    }

    pub fn iter_clips(&self) -> impl Iterator<Item = (&ClipId, &Clip)> {
        self.clips.iter()
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

    /// 指定レイヤーの、その時刻に実際に存在する（`duration` を含めて判定）クリップを取得します。
    ///
    /// `Layer::get_clip_id_at` は開始位置しか見ていないため、範囲判定はここで行います。
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

    /// クリップの `position` を変更します。
    ///
    /// `layer.clips` と `chunk_index` の整合性を保つ唯一の経路です。
    pub fn move_clip(&mut self, clip_id: ClipId, new_position: i64) -> EsotereelResult<()> {
        let layer_id = self
            .layers
            .values()
            .find(|l| l.clips.values().any(|&id| id == clip_id))
            .map(|l| l.id)
            .ok_or_else(|| anyhow::anyhow!("Clip not found in any layer"))?;

        let layer = self.layers.get_mut(&layer_id).unwrap();
        layer
            .remove_clip(clip_id)
            .ok_or(EsotereelError::LayerNotFound(layer_id))?;
        layer.clips.insert(new_position, clip_id);

        if let Some(clip) = self.clips.get_mut(&clip_id) {
            clip.set_position(new_position);
        }

        self.touch_upsert(clip_id);

        Ok(())
    }

    /// 指定範囲に重なるクリップを検索します。
    ///
    /// `chunk_index` が無ければ遅延構築します。
    pub fn query_range(&self, range: Range<i64>) -> Vec<(&Layer, &Clip)> {
        // Single lock strategy: use write lock to handle both check and build
        // This avoids read-write lock upgrade deadlock
        let Ok(mut idx_guard) = self.chunk_index.write() else {
            log::warn!("timeline: chunk_index write lock poisoned");
            return Vec::new();
        };

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

    pub fn touch_property_changed(
        &mut self,
        clip_id: ClipId,
        key: NamespacedID,
        value: ClipBindingValue,
    ) {
        self.invalidate_index();
        self.changes.mark_clip_property_changed(clip_id, key, value);
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

    // Independent化(deep clone)

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

    // クライアント側: ネットワーク経由の構築/マージ

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
                        clips: std::collections::BTreeMap::new(),
                    },
                )
            })
            .collect();

        Self {
            id: meta.id,
            tps: meta.fps,
            layers,
            outline: meta.outline.clone(),
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
        // レイヤー跨ぎ移動に対応するため、全レイヤーからこのClipIdの古い参照を消去する
        for layer in self.layers.values_mut() {
            layer.clips.retain(|_, &mut cid| cid != clip.id);
        }

        //  移動先のレイヤーへ配置する
        if let Some(layer) = self.layers.get_mut(&layer_id) {
            layer.clips.insert(clip.position, clip.id);
        }

        // Clip実体を更新・保持する
        self.clips.insert(clip.id, clip);
        self.invalidate_index();
    }

    pub fn apply_outline_folder_meta(&mut self, id: LayerFolderId, meta: Meta) {
        self.outline
            .upsert_folder_meta(id, meta.name, meta.opacity, meta.blend_mode);
    }

    pub fn apply_outline_children(
        &mut self,
        parent: Option<LayerFolderId>,
        children: Vec<OutlineNode>,
    ) {
        self.outline.set_children(parent, children);
    }

    pub fn remove_outline_folder_local(&mut self, id: LayerFolderId) {
        self.outline.remove_folder_entry_only(id);
    }
}

/// ProjectAll用の軽量版。Clip本体を含まない。
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct TimelineMeta {
    pub id: u64,
    pub fps: f64,
    pub layers: Vec<LayerMeta>,
    pub outline: LayerOutline,
}

impl From<&Timeline> for TimelineMeta {
    fn from(tl: &Timeline) -> Self {
        Self {
            id: tl.id,
            fps: tl.tps,
            layers: tl.layers_meta(),
            outline: tl.outline.clone(),
        }
    }
}
