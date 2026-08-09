use rkyv::{
    Archive, CheckBytes, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize, bytecheck,
};
use serde::{Deserialize, Serialize};

use crate::project::{ids::TimelineId, transform::ClipTranslates};

#[derive(Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Clip {
    pub id: u64,
    pub position: i64,
    pub duration: i64,
    pub data: ClipData,
    pub translates: ClipTranslates,
}

impl Clip {
    pub fn new(
        id: u64,
        position: i64,
        duration: i64,
        clip_data: ClipData,
        translates: ClipTranslates,
    ) -> Self {
        Self {
            id,
            position,
            duration,
            data: clip_data,
            translates,
        }
    }

    pub fn position(&self) -> i64 {
        self.position
    }

    pub fn set_position(&mut self, new_pos: i64) {
        self.position = new_pos;
    }
}

#[derive(Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub enum ClipData {
    Dummy,
    Video { path: String, media_offset: f64 },
    Audio { path: String, media_offset: f64 },
    Composite { timeline_id: Option<TimelineId> },
    Area2D { timeline_id: Option<TimelineId> },
    Area3D { timeline_id: Option<TimelineId> },
}

impl ClipData {
    pub fn get_media_seconds(
        global_fps: f64,
        clip_position: i64,
        current_frame: i64,
        media_offset: f64,
    ) -> f64 {
        let relative_frame = current_frame - clip_position;
        if relative_frame < 0 {
            return media_offset;
        }
        (relative_frame as f64 / global_fps) + media_offset
    }
}

impl PartialEq for Clip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Clip {}

impl PartialEq for ArchivedClip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for ArchivedClip {}
