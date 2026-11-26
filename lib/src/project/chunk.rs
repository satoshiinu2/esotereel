//! 時間範囲で区切ったクリップの所有単位（チャンク）です。
//!
//! クリップは開始位置が属するチャンクが実体を所有します。チャンク幅は
//! 保存形式（`save` モジュール）と揃えて `CHUNK_TICKS` です。
//!
//! チャンク内には実体（`Clip`）とは別に、全クリップ分の「配置」を常駐
//! させます。配置は所属レイヤー・duration・kind 参照（連番）だけの軽い
//! 情報で、実体がアンロードされても残り、ズームアウト描画や範囲検索の
//! 候補列挙に使えます。配置の正はここ（チャンク側）です。

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use rkyv::{Archive, CheckBytes, bytecheck};

use crate::project::clip::Clip;
use crate::project::ids::{ClipId, LayerId};
use crate::project::kind_table::KindId;
use crate::util::result::EsotereelResult;

/// チャンクの連番です。開始 tick を `CHUNK_TICKS` で割った値です。
pub type ChunkId = i64;

/// 1チャンクの時間幅（tick）です。保存形式のチャンク幅と一致させます。
pub const CHUNK_TICKS: i64 = 30_000;

/// 開始位置が属するチャンクの連番を返します。
pub fn chunk_of(position: i64) -> ChunkId {
    position.div_euclid(CHUNK_TICKS)
}

/// チャンクの読み込み状態です。
///
/// `Loading` は読み込み処理の最中だけの遷移状態で、読み込みが終われば
/// `Loaded`、失敗すれば `Unloaded` に戻ります。再入した読み込み要求は
/// この状態を見て弾かれます。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
)]
#[archive_attr(derive(CheckBytes))]
pub enum ChunkState {
    /// 実体がメモリ上にあります。
    #[default]
    Loaded,
    /// 実体を読み込み中です。
    Loading,
    /// 実体は外部（`ChunkStore`）にあり、配置だけが残っています。
    Unloaded,
}

/// チャンク内に常駐するクリップ1件分の配置です。
///
/// 開始位置は保持側（`Chunk.placements`）のキーが持ちます。実体が
/// アンロードされてもこの情報は残ります。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[archive_attr(derive(CheckBytes))]
pub struct ChunkPlacement {
    pub clip_id: ClipId,
    pub layer_id: LayerId,
    pub duration: i64,
    /// `KindTable` で intern した連番。文字列の kind は実体側が持ちます。
    pub kind: KindId,
}

impl ChunkPlacement {
    /// 配置の終了 tick（開始位置＋duration）を返します。
    pub fn end(&self, position: i64) -> i64 {
        position + self.duration
    }
}

/// クリップ実体と配置を所有するチャンクです。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct Chunk {
    state: ChunkState,
    /// （開始位置, ClipId）→ 配置。全クリップ分が常駐します。
    /// 別レイヤーなら同じ開始位置に複数のクリップが存在できるため、
    /// キーは位置だけでは足りません。
    placements: BTreeMap<(i64, ClipId), ChunkPlacement>,
    /// クリップ実体。`Loaded` のときだけ中身があります。
    clips: HashMap<ClipId, Clip>,
    /// このチャンク内のクリップの終了 tick の最大値です。
    /// チャンクをまたぐ長いクリップを範囲検索で拾うために使います。
    max_end: i64,
    /// 最後に読み込まれてから実体が変更されたかどうかです。
    dirty: bool,
}

impl Chunk {
    /// 空のチャンク（`Loaded`）を作成します。
    pub fn new() -> Self {
        Self {
            state: ChunkState::Loaded,
            placements: BTreeMap::new(),
            clips: HashMap::new(),
            max_end: i64::MIN,
            dirty: false,
        }
    }

    /// 読み込み状態を返します。
    pub fn state(&self) -> ChunkState {
        self.state
    }

    /// 実体がメモリ上にあるかどうかを返します。
    pub fn is_loaded(&self) -> bool {
        self.state == ChunkState::Loaded
    }

    /// このチャンクに属するクリップ数（配置の数）を返します。
    pub fn clip_count(&self) -> usize {
        self.placements.len()
    }

    /// このチャンク内のクリップの終了 tick の最大値を返します。
    pub fn max_end(&self) -> i64 {
        self.max_end
    }

    /// 実体が最後の読み込み以降に変更されたかどうかを返します。
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 配置を反復します（開始位置の昇順）。
    pub fn placements(&self) -> impl Iterator<Item = (i64, &ChunkPlacement)> {
        self.placements.iter().map(|(&(pos, _), p)| (pos, p))
    }

    /// クリップ ID から配置と開始位置を引きます。
    pub fn placement_of(&self, clip_id: ClipId) -> Option<(i64, &ChunkPlacement)> {
        self.placements
            .iter()
            .find(|(_, p)| p.clip_id == clip_id)
            .map(|(&(pos, _), p)| (pos, p))
    }

    /// クリップ実体を引きます。アンロード中は `None` です。
    pub fn clip(&self, clip_id: ClipId) -> Option<&Clip> {
        self.clips.get(&clip_id)
    }

    /// クリップ実体を可変で引きます。アンロード中は `None` です。
    ///
    /// 実体を触る経路は変更ありとして dirty を立てます。
    pub fn clip_mut(&mut self, clip_id: ClipId) -> Option<&mut Clip> {
        if !self.is_loaded() {
            return None;
        }
        self.dirty = true;
        self.clips.get_mut(&clip_id)
    }

    /// 実体を反復します。アンロード中は空です。
    pub fn clips(&self) -> impl Iterator<Item = &Clip> {
        self.clips.values()
    }

    /// 配置と実体を登録します。
    ///
    /// 配置の開始位置・duration は実体と一致している前提です
    /// （呼び出し側の `Timeline` が保証します）。実体を保持するため、
    /// チャンクは `Loaded` でなければなりません（アンロード中のチャンクへは
    /// 先に読み込んでから登録します）。
    pub fn insert(&mut self, placement: ChunkPlacement, clip: Clip) {
        debug_assert!(self.is_loaded());
        let position = clip.position;
        debug_assert_eq!(placement.clip_id, clip.id);
        debug_assert_eq!(placement.duration, clip.duration);
        self.max_end = self.max_end.max(placement.end(position));
        self.placements.insert((position, placement.clip_id), placement);
        self.clips.insert(clip.id, clip);
        self.dirty = true;
    }

    /// 配置と実体を取り除き、（開始位置、配置、実体）を返します。
    ///
    /// アンロード中で実体が無い場合は配置だけを取り除き、実体は `None` です。
    pub fn remove(&mut self, clip_id: ClipId) -> Option<(i64, ChunkPlacement, Option<Clip>)> {
        let (position, placement) = self.placement_of(clip_id)?;
        let placement = *placement;
        self.placements.remove(&(position, clip_id));
        if placement.end(position) >= self.max_end {
            self.recompute_max_end();
        }
        let clip = self.clips.remove(&clip_id);
        if clip.is_some() {
            self.dirty = true;
        }
        Some((position, placement, clip))
    }

    /// 配置だけを登録します（アンロード中のクリップの配置移動用）。
    ///
    /// 実体は外部にある前提で、配置と `max_end` だけを更新します。
    pub fn insert_placement(&mut self, placement: ChunkPlacement, position: i64) {
        self.max_end = self.max_end.max(placement.end(position));
        self.placements
            .insert((position, placement.clip_id), placement);
        self.dirty = true;
    }

    /// 配置だけを差し替えます（位置・duration・所属レイヤーの変更用）。
    ///
    /// 実体の `position` は呼び出し側が合わせて変更します。
    pub fn replace_placement(
        &mut self,
        clip_id: ClipId,
        new_position: i64,
        new_placement: ChunkPlacement,
    ) -> Option<()> {
        let (old_position, old) = self.placement_of(clip_id)?;
        let was_max = old.end(old_position) >= self.max_end;
        self.placements.remove(&(old_position, clip_id));
        self.placements
            .insert((new_position, clip_id), new_placement);
        if was_max {
            self.recompute_max_end();
        } else {
            self.max_end = self.max_end.max(new_placement.end(new_position));
        }
        self.dirty = true;
        Some(())
    }

    fn recompute_max_end(&mut self) {
        self.max_end = self
            .placements
            .iter()
            .map(|(&(pos, _), p)| p.end(pos))
            .max()
            .unwrap_or(i64::MIN);
    }

    /// 配置が持つ kind 連番を対応表で振り直します（テーブル統合用）。
    /// 対応表に無い連番はそのまま残します（呼び出し側が完全な対応表を
    /// 渡す前提です）。
    pub(crate) fn remap_kinds(&mut self, map: &std::collections::HashMap<KindId, KindId>) {
        for placement in self.placements.values_mut() {
            if let Some(&new_kind) = map.get(&placement.kind) {
                placement.kind = new_kind;
            }
        }
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    /// 読み込み状態を切り替えます（`Timeline` の読み込み制御用）。
    pub(crate) fn set_state(&mut self, state: ChunkState) {
        self.state = state;
    }

    /// 実体をすべて取り出します（アンロード時の書き出し用）。
    pub(crate) fn take_clips(&mut self) -> Vec<Clip> {
        std::mem::take(&mut self.clips).into_values().collect()
    }

    /// 実体を丸ごと差し替えます（読み込み完了時の反映用）。
    pub(crate) fn set_clips(&mut self, clips: Vec<Clip>) {
        self.clips = clips.into_iter().map(|clip| (clip.id, clip)).collect();
    }

    /// dirty を下ろします（読み込み・書き出しが完了した時点で呼びます）。
    pub(crate) fn mark_clean(&mut self) {
        self.dirty = false;
    }
}

/// チャンク実体の外部ストレージです。
///
/// アンロードした実体の行き先と、再読み込みの取得元を抽象化します。
/// 実装はプロジェクトフォルダのチャンクファイルなど、実体を
/// チャンク単位で出し入れできるものを想定します。
pub trait ChunkStore: std::fmt::Debug + Send + Sync {
    /// チャンクの実体をすべて読み込みます。
    ///
    /// 返した実体は、そのチャンクの配置（レイヤー・位置・duration・kind）と
    /// 一致していなければならず、`Timeline` 側で検証されます。
    fn load_chunk(&mut self, chunk_id: ChunkId) -> EsotereelResult<Vec<(LayerId, Clip)>>;

    /// チャンクの実体をすべて書き出します。
    fn store_chunk(
        &mut self,
        chunk_id: ChunkId,
        clips: &[(LayerId, Clip)],
    ) -> EsotereelResult<()>;
}

/// チャンクのアンロード方針です。設定システムへの接続は別途行い、
/// ここでは差し替え可能な値として持ちます。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkResidencyConfig {
    /// 最後に使われてからこの時間を超えたチャンクをアンロード対象にします。
    pub idle_timeout: Duration,
    /// 累積アクセス数がこれ以上のチャンクは、時間が経っても保持します
    /// （頻繁に使われるチャンクを読み込み続けるため）。0 なら無効です。
    pub frequent_access_threshold: u64,
}

impl Default for ChunkResidencyConfig {
    fn default() -> Self {
        Self {
            // クリップ実体は動画フレームより軽いため、閾値は大目に取る。
            idle_timeout: Duration::from_secs(300),
            frequent_access_threshold: 16,
        }
    }
}
