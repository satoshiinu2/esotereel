use crate::plugin::NamespacedID;
use crate::project::{
    clip::ClipBindingValue,
    ids::{ClipId, LayerFolderId, LayerId},
};
use std::collections::{HashMap, HashSet};

/// 削除されたクリップの情報です。
#[derive(Debug, Clone)]
pub struct RemovedClipInfo {
    pub layer_id: LayerId,
    pub position: i64,
    pub duration: i64,
}

/// クリッププロパティの変更です。
#[derive(Debug, Clone)]
pub struct ClipPropertyChange {
    pub clip_id: ClipId,
    pub key: NamespacedID,
    pub value: ClipBindingValue,
}

/// 未同期の差分です。
///
/// `drain_changes()` で取り出されるまで蓄積され続けます。
/// クリップ、レイヤー、アウトライン（フォルダー）の各種変更を追跡します。
#[derive(Debug, Default, Clone)]
pub struct ChangeSet {
    /// 追加・更新されたクリップ ID の集合
    pub clips_upserted: HashSet<ClipId>,
    /// 削除されたクリップ ID とその情報
    pub clips_removed: HashMap<ClipId, RemovedClipInfo>,
    /// 変更されたクリッププロパティのリスト
    pub clip_properties_changed: Vec<ClipPropertyChange>,

    /// 追加・更新されたレイヤー ID の集合
    pub layers_upserted: HashSet<LayerId>,
    /// 削除されたレイヤー ID の集合
    pub layers_removed: HashSet<LayerId>,
    /// ルートレイヤーが変更されたかどうか
    pub root_layers_changed: bool,

    /// 追加・更新されたフォルダー ID の集合
    pub outline_folders_upserted: HashSet<LayerFolderId>,
    /// 子変更が発生したフォルダー ID（親フォルダー）の集合
    pub outline_children_changed: HashSet<Option<LayerFolderId>>,
    /// 削除されたフォルダー ID の集合
    pub outline_folders_removed: HashSet<LayerFolderId>,
}

impl ChangeSet {
    /// すべての変更カテゴリが空かどうかを返します。
    pub fn is_empty(&self) -> bool {
        self.is_clip_empty() && self.is_layer_empty() && self.is_outline_empty()
    }

    /// クリップ関連の変更が空かどうかを返します。
    pub fn is_clip_empty(&self) -> bool {
        self.clips_upserted.is_empty()
            && self.clips_removed.is_empty()
            && self.clip_properties_changed.is_empty()
    }

    /// クリップを upsert としてマークします。
    pub(crate) fn mark_clip_upserted(&mut self, id: ClipId) {
        self.clips_removed.remove(&id);
        self.clips_upserted.insert(id);
    }

    /// クリップを削除としてマークします。
    pub(crate) fn mark_clip_removed(&mut self, id: ClipId, info: RemovedClipInfo) {
        self.clips_upserted.remove(&id);
        self.clips_removed.insert(id, info);
    }

    /// クリッププロパティの変更を記録します。
    pub(crate) fn mark_clip_property_changed(
        &mut self,
        clip_id: ClipId,
        key: NamespacedID,
        value: ClipBindingValue,
    ) {
        self.clip_properties_changed.push(ClipPropertyChange {
            clip_id,
            key,
            value,
        });
    }

    /// レイヤー関連の変更が空かどうかを返します。
    pub fn is_layer_empty(&self) -> bool {
        self.layers_upserted.is_empty()
            && self.layers_removed.is_empty()
            && !self.root_layers_changed
    }

    /// レイヤーを upsert としてマークします。
    pub(crate) fn mark_layer_upserted(&mut self, id: LayerId) {
        self.layers_removed.remove(&id);
        self.layers_upserted.insert(id);
    }

    /// レイヤーを削除としてマークします。
    pub(crate) fn mark_layer_removed(&mut self, id: LayerId) {
        self.layers_upserted.remove(&id);
        self.layers_removed.insert(id);
    }

    /// アウトライン関連の変更が空かどうかを返します。
    pub fn is_outline_empty(&self) -> bool {
        self.outline_folders_upserted.is_empty()
            && self.outline_children_changed.is_empty()
            && self.outline_folders_removed.is_empty()
    }

    /// フォルダーを upsert としてマークします。
    pub fn mark_outline_folder_upserted(&mut self, id: LayerFolderId) {
        self.outline_folders_upserted.insert(id);
    }

    /// フォルダーの子変更を記録します。
    pub fn mark_outline_children_changed(&mut self, parent: Option<LayerFolderId>) {
        self.outline_children_changed.insert(parent);
    }

    /// フォルダーを削除としてマークします。
    ///
    /// 削除されたフォルダーは upsert 集合からも除外されます（消えるものを
    /// upsert する意味はないため）。
    pub fn mark_outline_folder_removed(&mut self, id: LayerFolderId) {
        self.outline_folders_removed.insert(id);
        self.outline_folders_upserted.remove(&id);
    }

    /// 他の `ChangeSet` をマージします。
    pub fn merge(&mut self, other: ChangeSet) {
        for id in other.clips_upserted {
            self.mark_clip_upserted(id);
        }
        for (id, l) in other.clips_removed {
            self.mark_clip_removed(id, l);
        }

        for id in other.layers_upserted {
            self.mark_layer_upserted(id);
        }
        for id in other.layers_removed {
            self.mark_layer_removed(id);
        }
        self.root_layers_changed |= other.root_layers_changed;
    }
}
