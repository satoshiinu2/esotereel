use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::{
    project::{LayerMapKey, clip::ClipData, transform::ClipTranslates},
    util::types::ClipMoveCtx,
};

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Command {
    ClipsMove {
        clips: Vec<ClipMoveCtx>,
    },
    AddClip {
        layer_map_key: LayerMapKey,
        position: i64,
        duration: i64,
        clip_data: ClipData,
        translates: ClipTranslates,
    },
}
