//! 各種識別子の型エイリアスと ID 生成器を定義します。
//!
//! FFI を単純にするため、ニュータイプパターンは使わずプリミティブ型のエイリアスとします。

use rkyv::{
    Archive, CheckBytes, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize, bytecheck,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// タイムライン識別子（`u64`）。
pub type TimelineId = u64;
/// レイヤー識別子（`u64`）。
pub type LayerId = u64;
/// レイヤー移動量（`i64`）。
pub type LayerDelta = i64;
/// レイヤーフォルダー識別子（`u64`）。
pub type LayerFolderId = u64;
/// クリップ識別子（`u64`）。
pub type ClipId = u64;
/// スクリプト識別子（`u64`）。
pub type ScriptId = u64;
/// リソース識別子（`u32`）。
pub type ResourceId = u32;
/// アセット識別子（`Uuid`）。
pub type AssetId = Uuid;

/// 各種 ID の採番生成器です。
///
/// ロード時に `observe_*` メソッドで既存 ID を考慮し、衝突を避けます。
#[derive(
    Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Default, Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct IdGenerator {
    next_timeline: TimelineId,
    next_layer: LayerId,
    next_clip: ClipId,
    next_folder: LayerFolderId,
}

impl IdGenerator {
    /// 次のタイムライン ID を発行します。
    pub fn next_timeline_id(&mut self) -> TimelineId {
        let id = self.next_timeline;
        self.next_timeline += 1;
        id
    }
    /// 次のレイヤー ID を発行します。
    pub fn next_layer_id(&mut self) -> LayerId {
        let id = self.next_layer;
        self.next_layer += 1;
        id
    }
    /// 次のクリップ ID を発行します。
    pub fn next_clip_id(&mut self) -> ClipId {
        let id = self.next_clip;
        self.next_clip += 1;
        id
    }
    /// 次のフォルダー ID を発行します。
    pub fn next_folder_id(&mut self) -> LayerFolderId {
        let id = self.next_folder;
        self.next_folder += 1;
        id
    }

    /// ロード時、既存 ID と衝突しないようカウンタを引き上げます。
    pub fn observe_timeline(&mut self, id: TimelineId) {
        self.next_timeline = self.next_timeline.max(id + 1);
    }
    /// ロード時、既存レイヤー ID と衝突しないようカウンタを引き上げます。
    pub fn observe_layer(&mut self, id: LayerId) {
        self.next_layer = self.next_layer.max(id + 1);
    }
    /// ロード時、既存クリップ ID と衝突しないようカウンタを引き上げます。
    pub fn observe_clip(&mut self, id: ClipId) {
        self.next_clip = self.next_clip.max(id + 1);
    }
    /// ロード時、既存フォルダー ID と衝突しないようカウンタを引き上げます。
    pub fn observe_folder(&mut self, id: LayerFolderId) {
        self.next_folder = self.next_folder.max(id + 1);
    }
}
