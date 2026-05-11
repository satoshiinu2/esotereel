use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::{
    project::{clip_data::ClipData, clip_translate::ClipTranslates},
    util::types::ClipMoveCtx,
};

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Command {
    ClipsMove {
        clips: Vec<ClipMoveCtx>,
    },
    AddClip {
        layer_idx: usize,
        position: i64,
        duration: i64,
        clip_data: ClipData,
        translates: ClipTranslates,
    },
}
