use crate::plugin::property::PropertyPath;
use std::collections::HashMap;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::{
    plugin::NamespacedID,
    project::{
        TimelineTick,
        clip::ClipBindingValue,
        ids::{ClipId, LayerFolderId, LayerId},
    },
};

/// クリップ移動のコンテキストです。
///
/// `base_position`/`base_duration` はクライアントが認識している現在の値で、
/// ステールネス（古い状態に基づく操作）検出に使用されます。
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct ClipMoveCtx {
    pub clip_id: ClipId,
    /// クライアントが認識している現在位置（ステールネス検出用）
    pub base_position: TimelineTick,
    /// クライアントが認識している現在長さ（ステールネス検出用）
    pub base_duration: TimelineTick,
    pub new_position: TimelineTick,
    pub new_duration: TimelineTick,
    pub new_layer_id: LayerId,
}

/// クリップ移動の履歴コンテキストです。Undo/Redo で使用します。
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct ClipMoveHistoryCtx {
    pub clip_id: ClipId,
    pub old_position: TimelineTick,
    pub old_duration: TimelineTick,
    pub old_layer_id: LayerId,
    pub new_position: TimelineTick,
    pub new_duration: TimelineTick,
    pub new_layer_id: LayerId,
}

/// クリップリサイズのコンテキストです。
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct ClipResizeCtx {
    pub clip_id: ClipId,
    pub left_edge: bool,
    /// クライアントが認識している現在位置（ステールネス検出用）
    pub base_position: TimelineTick,
    /// クライアントが認識している現在長さ（ステールネス検出用）
    pub base_duration: TimelineTick,
    pub new_position: TimelineTick,
    pub new_duration: TimelineTick,
}

/// クリップリサイズの履歴コンテキストです。Undo/Redo で使用します。
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct ClipResizeHistoryCtx {
    pub clip_id: ClipId,
    pub left_edge: bool,
    pub old_position: TimelineTick,
    pub old_duration: TimelineTick,
    pub new_position: TimelineTick,
    pub new_duration: TimelineTick,
    /// サーバーが計算した位置変化量（リモート適用・競合検出用）
    pub position_delta: TimelineTick,
    /// サーバーが計算した長さ変化量（リモート適用・競合検出用）
    pub duration_delta: TimelineTick,
}

/// タイムライン操作のコマンドリクエストです。
///
/// クライアントからサーバーへ送られ、`Request::Command` でバッチ処理されます。
/// 各バリアントは「どのような編集を行うか」を表現し、サーバー側で
/// 重複チェックやステールネス検出を行った上で適用します。
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub enum CommandRequest {
    /// 複数クリップを一括移動します。
    ClipsMove {
        clips: Vec<ClipMoveCtx>,
    },
    /// 複数クリップを一括リサイズします。
    ClipsResize {
        clips: Vec<ClipResizeCtx>,
    },
    /// 新しいクリップを追加します。
    AddClip {
        layer_id: LayerId,
        position: TimelineTick,
        duration: TimelineTick,

        kind_id: NamespacedID,
        properties: HashMap<NamespacedID, ClipBindingValue>,
    },
    /// 新しいレイヤーを追加します。
    AddLayer {
        parent_folder_id: Option<LayerFolderId>,
        insert_index: Option<usize>,
        name: String,
    },
    /// 新しいフォルダーを追加します。
    AddFolder {
        parent_folder_id: Option<LayerFolderId>,
        insert_index: Option<usize>,
        name: String,
    },
    /// クリッププロパティの値を設定します。
    SetClipPropertyValue {
        clip_id: ClipId,
        key: PropertyPath,
        value: ClipBindingValue,
    },
}

/// コマンドの履歴エントリです。Undo/Redo で使用します。
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub enum CommandHistory {
    /// クリップ移動の履歴。
    ClipsMove {
        clips: Vec<ClipMoveHistoryCtx>,
    },
    /// クリップリサイズの履歴。
    ClipsResize {
        clips: Vec<ClipResizeHistoryCtx>,
    },
    /// クリップ追加の履歴。
    AddClip {
        clip_id: Option<ClipId>,
        layer_id: LayerId,
        position: TimelineTick,
        duration: TimelineTick,

        kind_id: NamespacedID,
        properties: HashMap<NamespacedID, ClipBindingValue>,
    },
    /// レイヤー追加の履歴。
    AddLayer {
        layer_id: Option<LayerId>,
        parent_layer_id: Option<LayerId>,
        insert_index: Option<usize>,
        name: String,
    },
    /// フォルダー追加の履歴。
    AddFolder {
        folder_id: Option<LayerFolderId>,
        parent_layer_id: Option<LayerId>,
        insert_index: Option<usize>,
        name: String,
    },
    /// プロパティ値変更の履歴。
    SetClipPropertyValue {
        clip_id: ClipId,
        key: PropertyPath,
        old_value: ClipBindingValue,
        new_value: ClipBindingValue,
    },
}
