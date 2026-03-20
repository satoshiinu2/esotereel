use crate::ui::timeline::TimelineType;

pub struct Clip {
    pub id: u64,
    pub position: i64,
    pub duration: i64,
}

pub struct ClipDragState {
    pub src_timeline_type: TimelineType,
    pub src_layer_idx: usize,
    pub clip_idx: usize,
    pub offset_frames: i64,
    pub current_timeline_type: TimelineType,
    pub current_layer_idx: usize,
    pub current_frame: i64,
    pub ghost_pos: egui::Pos2,
}
