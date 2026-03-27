use rkyv::{Archive, Deserialize, Serialize};

use crate::project::clip::Clip;

#[repr(C)]
pub struct ClipLocation {
    pub layer_idx: usize,
    pub clip_idx: usize,
    pub clip: *const Clip,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[repr(C)]
pub struct ClipMoveCtx {
    pub clip_id: u64,
    pub new_frame: u64,
    pub new_layer: u32,
}
