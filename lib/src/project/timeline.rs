use rkyv::{bytecheck, CheckBytes};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use crate::plugin::{property::PropertyKey, NamespacedID};
use crate::project::change::{ChangeSet, RemovedClipInfo};
use crate::project::chunk::{chunk_of, Chunk, ChunkId, ChunkPlacement};
use crate::project::clip::Clip;
use crate::project::clip::ClipBindingValue;
use crate::project::ids::{ClipId, IdGenerator, LayerFolderId, LayerId};
use crate::project::kind_table::{KindId, KindTable};
use crate::project::layer::{Layer, LayerMeta, LayerRemoveStrategy};
use crate::project::layer_outline::{LayerFolder, LayerOutline, Meta, OutlineNode};
use crate::project::TimelineTick;
use crate::util::result::{EsotereelError, EsotereelResult};

/// Composite / Script / Mirror の入れ子実行を無限ループさせないための上限。
pub const MAX_NESTED_DEPTH: u32 = 32;

/// 配置層から見たクリップ1件分の配置情報です。
///
/// 実体（`Clip`）がアンロードされていてもこの情報は引けます。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipPlacement {
    pub layer_id: LayerId,
    pub position: i64,
    pub duration: i64,
    /// `KindTable` で intern した連番です。
    pub kind: KindId,
}

/// タイムラインの実体です。
///
/// レイヤー（`Layer`）、クリップ、アウトライン（`LayerOutline`）を保持し、
/// 編集操作と変更セット（`ChangeSet`）の管理を行います。
///
/// - クリップ実体は時間チャンク（`Chunk`）が所有します。チャンク内には
///   全クリップ分の配置（所属レイヤー・duration・kind 参照）が常駐し、
///   実体がアンロードされても配置だけは残ります。
/// - `locator` は `ClipId` → `ChunkId` の引きで、所属レイヤーも配置側から
///   引くため、レイヤー横断の線形スキャンは行いません。
/// - `Layer.clips` はレイヤー内の位置 → `ClipId` の順序索引です。
/// - `changes` は未同期の差分で、ネットワーク同期時に `drain_changes` で取り出されます。
#[derive(
    rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde:: Deserialize, Debug,
)]
#[archive_attr(derive(CheckBytes))]
pub struct Timeline {
    pub id: u64,
    pub tps: f64,
    layers: HashMap<LayerId, Layer>,

    /// クリップ実体と配置の所有者。開始位置が属するチャンクに分かれます。
    chunks: BTreeMap<ChunkId, Chunk>,

    /// `ClipId` → 所属チャンクのロケータ。
    locator: HashMap<ClipId, ChunkId>,

    /// 配置が参照する kind 連番の intern テーブル。
    kind_table: KindTable,

    pub outline: LayerOutline,

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
            chunks: self.chunks.clone(),
            locator: self.locator.clone(),
            kind_table: self.kind_table.clone(),
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
            chunks: BTreeMap::new(),
            locator: HashMap::new(),
            kind_table: KindTable::default(),
            changes: ChangeSet::default(),
            outline: LayerOutline::default(),
        };

        for i in 0..4 {
            tl.insert_layer(ids, format!("Layer {}", i + 1), None, None)
                .expect("inserting a layer at the root cannot fail");
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
            chunks: BTreeMap::new(),
            locator: HashMap::new(),
            kind_table: KindTable::default(),
            changes: ChangeSet::default(),
        }
    }

    // Layer

    pub fn insert_layer(
        &mut self,
        ids: &mut IdGenerator,
        name: String,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
    ) -> EsotereelResult<LayerId> {
        // 親フォルダの存在を先に確認する。挿入後に失敗すると
        // layers と outline が不整合なまま残ってしまう。
        if let Some(folder_id) = parent {
            if !self.outline.has_folder(folder_id) {
                return Err(EsotereelError::LayerFolderNotFound(folder_id).into());
            }
        }
        let id = ids.next_layer_id();
        self.layers.insert(id, Layer::new(id, name));
        self.changes.mark_layer_upserted(id);

        self.outline.insert_layer(id, parent, index);
        self.changes.mark_outline_children_changed(parent);

        Ok(id)
    }

    pub fn insert_folder(
        &mut self,
        ids: &mut IdGenerator,
        name: String,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
    ) -> EsotereelResult<LayerFolderId> {
        if let Some(folder_id) = parent {
            if !self.outline.has_folder(folder_id) {
                return Err(EsotereelError::LayerFolderNotFound(folder_id).into());
            }
        }
        let id = ids.next_folder_id();
        self.outline.insert_folder(id, name, parent, index);
        self.changes.mark_outline_folder_upserted(id);
        self.changes.mark_outline_children_changed(parent);
        Ok(id)
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

    // Clip: 配置とロケータ

    /// クリップの配置を返します。実体がアンロードされていても引けます。
    pub fn get_placement(&self, clip_id: ClipId) -> Option<ClipPlacement> {
        let chunk = self.chunks.get(self.locator.get(&clip_id)?)?;
        let (position, placement) = chunk.placement_of(clip_id)?;
        Some(ClipPlacement {
            layer_id: placement.layer_id,
            position,
            duration: placement.duration,
            kind: placement.kind,
        })
    }

    /// クリップの所属レイヤーを返します。実体がアンロードされていても引けます。
    pub fn layer_of_clip(&self, clip_id: ClipId) -> Option<LayerId> {
        self.get_placement(clip_id).map(|p| p.layer_id)
    }

    /// kind の intern テーブルを返します。配置が持つ連番の解決に使います。
    pub fn kind_table(&self) -> &KindTable {
        &self.kind_table
    }

    /// 全クリップの配置を反復します（チャンク順・開始位置順）。
    pub fn iter_placements(&self) -> impl Iterator<Item = (ClipId, ClipPlacement)> + '_ {
        self.chunks.values().flat_map(|chunk| {
            chunk.placements().map(|(position, placement)| {
                (
                    placement.clip_id,
                    ClipPlacement {
                        layer_id: placement.layer_id,
                        position,
                        duration: placement.duration,
                        kind: placement.kind,
                    },
                )
            })
        })
    }

    /// 実体と配置をチャンクへ登録します。レイヤーの順序索引は触りません。
    fn insert_body(&mut self, layer_id: LayerId, clip: Clip) {
        let kind = self.kind_table.intern(&clip.kind_id);
        let placement = ChunkPlacement {
            clip_id: clip.id,
            layer_id,
            duration: clip.duration,
            kind,
        };
        let chunk_id = chunk_of(clip.position);
        self.chunks
            .entry(chunk_id)
            .or_default()
            .insert(placement, clip);
        self.locator.insert(placement.clip_id, chunk_id);
    }

    /// 配置と実体を取り除きます。レイヤーの順序索引は触りません。
    ///
    /// 戻り値は（実体、開始位置、配置）。アンロード中で実体が無い場合は
    /// 実体が `None` です。
    fn take_body(&mut self, clip_id: ClipId) -> Option<(Option<Clip>, i64, ChunkPlacement)> {
        let chunk_id = self.locator.remove(&clip_id)?;
        let (removed, empty) = {
            let chunk = self.chunks.get_mut(&chunk_id)?;
            let removed = chunk.remove(clip_id)?;
            let empty = chunk.clip_count() == 0;
            (removed, empty)
        };
        if empty {
            self.chunks.remove(&chunk_id);
        }
        let (position, placement, clip) = removed;
        Some((clip, position, placement))
    }

    // Clip: 作成・削除・配置

    pub fn new_clip_in(
        &mut self,
        layer_id: LayerId,
        ids: &mut IdGenerator,
        position: TimelineTick,
        duration: TimelineTick,
        kind_id: NamespacedID,
        properties: HashMap<PropertyKey, ClipBindingValue>,
    ) -> EsotereelResult<ClipId> {
        // 重複チェック(既存 try_insert 相当)
        {
            let layer = self
                .layers
                .get(&layer_id)
                .ok_or(EsotereelError::LayerNotFound(layer_id))?;
            let new_end = position + duration;
            for (&pos, &cid) in &layer.clips {
                let Some(existing) = self.get_placement(cid) else {
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
        self.insert_body(layer_id, clip);

        self.touch_upsert(clip_id);

        Ok(clip_id)
    }

    pub fn remove_clip_by_id(&mut self, clip_id: ClipId) -> Option<(Clip, LayerId)> {
        let layer_id = self.layer_of_clip(clip_id)?;
        let clip = self.remove_clip_by_id_in(layer_id, clip_id)?;
        Some((clip, layer_id))
    }

    pub fn remove_clip_by_id_in(&mut self, layer_id: LayerId, clip_id: ClipId) -> Option<Clip> {
        // 配置が示す所属レイヤーと異なる場合は、そのレイヤーには存在しない。
        if self.layer_of_clip(clip_id) != Some(layer_id) {
            return None;
        }
        let layer = self.layers.get_mut(&layer_id)?;
        layer.remove_clip(clip_id)?;

        let (clip, _, _) = self.take_body(clip_id)?;
        let clip = clip?; // アンロード中の実体は呼び出し側へ返せない
        self.touch_removed(&clip, layer_id);

        Some(clip)
    }

    /// 既に ID 確定済みの `Clip` をそのままレイヤーに配置します。新規 ID は発行しません。
    ///
    /// Undo/Redo での復元や、位置変更・レイヤー移動を伴う再配置に使用します。
    /// 既に別の場所に存在する同 ID の場合は、まず参照を除去してから配置し直します。
    pub fn place_clip(&mut self, layer_id: LayerId, clip: Clip) -> EsotereelResult<()> {
        if !self.layers.contains_key(&layer_id) {
            return Err(EsotereelError::LayerNotFound(layer_id).into());
        }
        let clip_id = clip.id;
        let position = clip.position;
        if self.locator.contains_key(&clip_id) {
            self.remove_clip_by_id(clip_id);
        }
        let layer = self
            .layers
            .get_mut(&layer_id)
            .ok_or(EsotereelError::LayerNotFound(layer_id))?;
        layer.clips.insert(position, clip_id);
        self.insert_body(layer_id, clip);

        self.touch_upsert(clip_id);

        Ok(())
    }

    pub fn get_clip(&self, id: ClipId) -> Option<&Clip> {
        let chunk = self.chunks.get(self.locator.get(&id)?)?;
        chunk.clip(id)
    }

    /// Clipとその所属LayerIdを検索
    pub fn get_clip_and_layer(&self, clip_id: ClipId) -> Option<(&Clip, LayerId)> {
        let placement = self.get_placement(clip_id)?;
        let clip = self.get_clip(clip_id)?;
        Some((clip, placement.layer_id))
    }

    pub fn iter_clips(&self) -> impl Iterator<Item = (&ClipId, &Clip)> {
        self.chunks
            .values()
            .flat_map(|chunk| chunk.clips().map(|clip| (&clip.id, clip)))
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
            let Some(existing) = self.get_placement(cid) else {
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
    /// 判定は配置で行うため、実体がアンロードされていても範囲は判定できますが、
    /// 返せるのは読み込まれている実体だけです。
    pub fn get_clip_at(&self, layer_id: LayerId, pos: i64) -> Option<&Clip> {
        let layer = self.layers.get(&layer_id)?;
        let clip_id = layer.get_clip_id_at(pos)?;
        let placement = self.get_placement(clip_id)?;
        if pos >= placement.position + placement.duration {
            return None;
        }
        self.get_clip(clip_id)
    }

    pub fn iter_layers(&self) -> impl Iterator<Item = (&LayerId, &Layer)> {
        self.layers.iter()
    }

    pub fn iter_layers_mut(&mut self) -> impl Iterator<Item = (&LayerId, &mut Layer)> {
        self.layers.iter_mut()
    }

    pub fn get_clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        // clip内容の書き換えは配置に影響しない(position/durationを
        // 変える場合は move_clip / set_clip_placement 経由にすること)
        let chunk = self.chunks.get_mut(self.locator.get(&id)?)?;
        chunk.clip_mut(id)
    }

    /// クリップの `position` を変更します。
    ///
    /// レイヤーの順序索引・配置・チャンク所有の整合性を保つ唯一の経路です。
    /// 所属レイヤーは配置（ロケータ）から引くため線形スキャンは行いません。
    pub fn move_clip(&mut self, clip_id: ClipId, new_position: i64) -> EsotereelResult<()> {
        let placement = self
            .get_placement(clip_id)
            .ok_or_else(|| anyhow::anyhow!("Clip not found in any layer"))?;
        let layer_id = placement.layer_id;

        let layer = self
            .layers
            .get_mut(&layer_id)
            .ok_or(EsotereelError::LayerNotFound(layer_id))?;
        layer
            .remove_clip(clip_id)
            .ok_or(EsotereelError::LayerNotFound(layer_id))?;
        layer.clips.insert(new_position, clip_id);

        self.move_body(clip_id, layer_id, new_position, placement.duration);

        self.touch_upsert(clip_id);

        Ok(())
    }

    /// 外部の正解値でクリップの配置（`position` / `duration`）を上書きします。
    ///
    /// サーバーからの拒否通知など、ローカルの実体を正しい配置へ寄せるための
    /// 経路です。順序索引・配置・チャンク所有をまとめて更新します。
    /// 対象が無ければ `false` を返します。
    pub fn set_clip_placement(
        &mut self,
        clip_id: ClipId,
        position: i64,
        duration: i64,
    ) -> bool {
        let Some(current) = self.get_placement(clip_id) else {
            return false;
        };
        if current.position != position {
            if let Some(layer) = self.layers.get_mut(&current.layer_id) {
                layer.remove_clip(clip_id);
                layer.clips.insert(position, clip_id);
            }
        }
        self.move_body(clip_id, current.layer_id, position, duration);
        true
    }

    /// 実体と配置を新しい位置・durationへ移します。
    ///
    /// チャンクをまたぐ場合は実体ごと所属チャンクを移し替えます。
    /// アンロード中のクリップは配置だけを移します。
    fn move_body(
        &mut self,
        clip_id: ClipId,
        layer_id: LayerId,
        new_position: i64,
        new_duration: i64,
    ) {
        let Some(current) = self.get_placement(clip_id) else {
            return;
        };
        let new_placement = ChunkPlacement {
            clip_id,
            layer_id,
            duration: new_duration,
            kind: current.kind,
        };
        let old_chunk_id = chunk_of(current.position);
        let new_chunk_id = chunk_of(new_position);

        if old_chunk_id == new_chunk_id {
            if let Some(chunk) = self.chunks.get_mut(&old_chunk_id) {
                chunk.replace_placement(clip_id, new_position, new_placement);
                if let Some(clip) = chunk.clip_mut(clip_id) {
                    clip.set_position(new_position);
                    clip.duration = new_duration;
                }
            }
            return;
        }

        // チャンクをまたぐ場合は実体ごと移し替える。アンロード中は配置だけが移る
        // （実体は外部にあり、次回読み込み時に配置と突き合わせて検証される）。
        let taken = self
            .chunks
            .get_mut(&old_chunk_id)
            .and_then(|chunk| chunk.remove(clip_id));
        if self
            .chunks
            .get(&old_chunk_id)
            .is_some_and(|chunk| chunk.clip_count() == 0)
        {
            self.chunks.remove(&old_chunk_id);
        }
        let Some((_, _, clip)) = taken else {
            return;
        };
        match clip {
            Some(mut clip) => {
                clip.set_position(new_position);
                clip.duration = new_duration;
                self.chunks
                    .entry(new_chunk_id)
                    .or_default()
                    .insert(new_placement, clip);
            }
            None => {
                self.chunks
                    .entry(new_chunk_id)
                    .or_default()
                    .insert_placement(new_placement, new_position);
            }
        }
        self.locator.insert(clip_id, new_chunk_id);
    }

    /// 指定範囲に重なるクリップを検索します。
    ///
    /// 候補はチャンク単位で列挙し、開始が範囲より前のチャンクにある長い
    /// クリップも拾うため、各チャンクの `max_end` を見て遡ります。
    /// 実体がアンロードされているクリップは返りません（配置だけなら
    /// `query_placements` を使います）。
    pub fn query_range(&self, range: Range<i64>) -> Vec<(&Layer, &Clip)> {
        let mut out = Vec::new();
        for chunk_id in self.candidate_chunk_ids(&range) {
            let Some(chunk) = self.chunks.get(&chunk_id) else {
                continue;
            };
            if !chunk.is_loaded() {
                continue;
            }
            for (position, placement) in chunk.placements() {
                if position >= range.end || placement.end(position) <= range.start {
                    continue;
                }
                let (Some(layer), Some(clip)) = (
                    self.layers.get(&placement.layer_id),
                    chunk.clip(placement.clip_id),
                ) else {
                    continue;
                };
                out.push((layer, clip));
            }
        }
        out
    }

    /// 指定範囲に重なるクリップの配置を検索します。
    ///
    /// 実体がアンロードされているクリップも含まれます。
    pub fn query_placements(&self, range: Range<i64>) -> Vec<(ClipId, ClipPlacement)> {
        let mut out = Vec::new();
        for chunk_id in self.candidate_chunk_ids(&range) {
            let Some(chunk) = self.chunks.get(&chunk_id) else {
                continue;
            };
            for (position, placement) in chunk.placements() {
                if position >= range.end || placement.end(position) <= range.start {
                    continue;
                }
                out.push((
                    placement.clip_id,
                    ClipPlacement {
                        layer_id: placement.layer_id,
                        position,
                        duration: placement.duration,
                        kind: placement.kind,
                    },
                ));
            }
        }
        out
    }

    /// 範囲に触れる可能性のあるチャンクの列を返します。
    ///
    /// 開始が範囲より前のチャンクにある長いクリップを取りこぼさないよう、
    /// `max_end` が範囲開始を超えるチャンクまで遡ります。
    fn candidate_chunk_ids(&self, range: &Range<i64>) -> Vec<ChunkId> {
        let end_c = chunk_of(range.end.saturating_sub(1).max(range.start));
        let mut start_c = chunk_of(range.start);
        while let Some((&prev, chunk)) = self.chunks.range(..start_c).next_back() {
            if chunk.max_end() > range.start {
                start_c = prev;
            } else {
                break;
            }
        }
        self.chunks
            .range(start_c..=end_c)
            .map(|(&id, _)| id)
            .collect()
    }

    pub(crate) fn touch_upsert(&mut self, id: ClipId) {
        self.changes.mark_clip_upserted(id);
    }

    pub fn touch_property_changed(
        &mut self,
        clip_id: ClipId,
        key: PropertyKey,
        value: ClipBindingValue,
    ) {
        self.changes.mark_clip_property_changed(clip_id, key, value);
    }

    pub(crate) fn touch_removed(&mut self, clip: &Clip, layer_id: LayerId) {
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
        for &cid in cloned.locator.keys() {
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
            chunks: BTreeMap::new(),
            locator: HashMap::new(),
            kind_table: KindTable::default(),
            changes: ChangeSet::default(),
        }
    }

    /// FetchClipsInRangeの結果をマージ。サーバーが確定させたClipをそのまま挿入するので
    /// IdGeneratorは触らない(new_clip_inとは別経路)。
    pub fn merge_fetched_clips(&mut self, entries: Vec<(LayerId, Clip)>) {
        for (layer_id, clip) in entries {
            // 既に保持しているクリップなら、旧所属側の参照と実体を先に外す
            // （チャンクをまたぐ移動で実体が二重にならないように）。
            if let Some(old) = self.get_placement(clip.id) {
                if let Some(old_layer) = self.layers.get_mut(&old.layer_id) {
                    old_layer.remove_clip(clip.id);
                }
                self.take_body(clip.id);
            }
            if let Some(layer) = self.layers.get_mut(&layer_id) {
                layer.clips.insert(clip.position, clip.id);
            }
            self.insert_body(layer_id, clip);
        }
    }

    /// ClipUpdates(差分同期)用: サーバー確定済みClipをupsertする。
    pub fn upsert_clip_from_network(&mut self, layer_id: LayerId, clip: Clip) {
        // レイヤー跨ぎ移動に対応するため、旧所属レイヤーからこのClipIdの参照を消去する
        if let Some(old) = self.get_placement(clip.id) {
            if let Some(old_layer) = self.layers.get_mut(&old.layer_id) {
                old_layer.clips.retain(|_, &mut cid| cid != clip.id);
            }
            self.take_body(clip.id);
        }

        //  移動先のレイヤーへ配置する
        if let Some(layer) = self.layers.get_mut(&layer_id) {
            layer.clips.insert(clip.position, clip.id);
        }

        // Clip実体を更新・保持する
        self.insert_body(layer_id, clip);
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

#[cfg(test)]
#[path = "../../tests/unit/project/timeline_tests.rs"]
mod tests;
