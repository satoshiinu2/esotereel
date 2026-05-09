use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
#[repr(u8)]
pub enum ClipData {
    Dummy,
    Video { path: String, media_offset: f64 },
    Audio { path: String, media_offset: f64 },
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

        let media_sec = (relative_frame as f64 / global_fps) + media_offset;

        media_sec
    }
}
