use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::project::{clip_data::ClipData, clip_translate::ClipTranslates};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Clip {
    pub id: u64,
    position: i64,
    pub duration: i64,
    pub clip_data: ClipData,
    pub translates: ClipTranslates,
}

impl Clip {
    pub fn position(&self) -> i64 {
        self.position
    }

    pub fn set_position(&mut self, v: i64) {
        self.position = v;
    }

    pub(in crate::project) unsafe fn new(
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
            clip_data,
            translates,
        }
    }
}

impl PartialEq for Clip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id // IDが同じなら同じクリップ
    }
}

impl PartialEq for ArchivedClip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id // IDが同じなら同じクリップ
    }
}

impl Eq for Clip {}
impl Eq for ArchivedClip {}
