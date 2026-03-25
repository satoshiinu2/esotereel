use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize)]
pub struct Project {
    pub timelines: [Timeline; 2],
    pub next_clip_id: u32,
}

#[derive(Archive, Deserialize, Serialize)]
pub struct Timeline {
    pub layers: Vec<Layer>,
    pub playhead: i64,
}

#[derive(Archive, Deserialize, Serialize)]
pub struct Layer {
    pub index: u32,
    pub clips: Vec<Clip>,
    pub name: String,
}

#[derive(Archive, Deserialize, Serialize)]
pub struct Clip {
    pub id: u32,
    pub position: i64,
    pub duration: i64,
}

#[derive(Archive, Deserialize, Serialize)]
pub struct ClipDragState {
    pub src_layer_idx: u32,
    pub src_frame: i64,
    pub clip_idx: u32,
    pub offset_frames: i64,
    pub current_layer_idx: u32,
    pub current_frame: i64,
    pub ghost_pos: [f64; 2],
}
