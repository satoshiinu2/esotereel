use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
#[repr(C)]
pub struct ClipMoveCtx {
    pub clip_id: u64,
    pub new_position: i64,
    pub new_duration: i64,
    pub new_layer_id: u64,
}

pub trait ClipMove {
    fn get_clip_id(&self) -> u64;
    fn get_new_position(&self) -> i64;
    fn get_new_duration(&self) -> i64;
}

impl ClipMove for ClipMoveCtx {
    fn get_clip_id(&self) -> u64 {
        self.clip_id
    }

    fn get_new_position(&self) -> i64 {
        self.new_position
    }

    fn get_new_duration(&self) -> i64 {
        self.new_duration
    }
}

impl ClipMove for ArchivedClipMoveCtx {
    fn get_clip_id(&self) -> u64 {
        self.clip_id
    }

    fn get_new_position(&self) -> i64 {
        self.new_position
    }

    fn get_new_duration(&self) -> i64 {
        self.new_duration
    }
}
