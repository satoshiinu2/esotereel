use std::collections::HashMap;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::{
    plugin::NamespacedID,
    project::{
        TimelineTick,
        clip::ClipBindingValue,
        ids::{ClipId, LayerFolderId, LayerId},
        transform::ClipTranslates,
    },
};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct ClipMoveCtx {
    pub clip_id: ClipId,
    pub new_position: TimelineTick,
    pub new_duration: TimelineTick,
    pub new_layer_id: LayerId,
}

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

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive_attr(derive(CheckBytes))]
pub enum CommandRequest {
    ClipsMove {
        clips: Vec<ClipMoveCtx>,
    },
    AddClip {
        layer_id: LayerId,
        position: TimelineTick,
        duration: TimelineTick,

        kind_id: NamespacedID,
        properties: HashMap<NamespacedID, ClipBindingValue>,

        translates: ClipTranslates,
    },
    AddLayer {
        parent_folder_id: Option<LayerFolderId>,
        insert_index: Option<usize>,
        name: String,
    },
    AddFolder {
        parent_folder_id: Option<LayerFolderId>,
        insert_index: Option<usize>,
        name: String,
    },
    SetClipPropertyValue {
        clip_id: ClipId,
        key: NamespacedID,
        value: ClipBindingValue,
    },
}

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive_attr(derive(CheckBytes))]
pub enum CommandHistory {
    ClipsMove {
        clips: Vec<ClipMoveHistoryCtx>,
    },
    AddClip {
        layer_id: LayerId,
        position: TimelineTick,
        duration: TimelineTick,

        kind_id: NamespacedID,
        properties: HashMap<NamespacedID, ClipBindingValue>,

        translates: ClipTranslates,
    },
    AddLayer {
        parent_layer_id: Option<LayerId>,
        insert_index: Option<usize>,
        name: String,
    },
    AddFolder {
        parent_layer_id: Option<LayerId>,
        insert_index: Option<usize>,
        name: String,
    },
    SetClipPropertyValue {
        clip_id: u64,
        key: NamespacedID,
        value: ClipBindingValue,
    },
}
