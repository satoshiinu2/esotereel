use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[archive_attr(derive(CheckBytes, Ord, PartialOrd, Eq, PartialEq))]
#[repr(u8)]
pub enum ClipData {
    Dummy,
    Video(String),
}

pub trait MediaClip<T = ClipData> {
    fn get_media_secounds(&self, global_fps: f64, clip_position: i64, timeline_frame: i64) -> f64 {
        let timeline_sec = timeline_frame as f64 / global_fps;

        let clip_start_sec = clip_position as f64 / global_fps;

        let media_start_sec = (timeline_sec - clip_start_sec).max(0.0);

        media_start_sec
    }
}
