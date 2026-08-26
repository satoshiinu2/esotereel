use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::{
    project::{clip::ClipData, ids::LayerId, transform::ClipTranslates},
    util::types::ClipMoveCtx,
};

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Command {
    ClipsMove {
        clips: Vec<ClipMoveCtx>,
    },
    AddClip {
        layer_id: LayerId,
        position: i64,
        duration: i64,
        clip_data: ClipData,
        translates: ClipTranslates,
    },
    AddLayer {
        parent_layer_id: Option<LayerId>,
        insert_index: Option<usize>,
        name: String,
        is_folder: bool,
    },
}
