use rkyv::{
    Archive, CheckBytes, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize, bytecheck,
};
use serde::{Deserialize, Serialize};

#[derive(
    Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Clone, PartialEq,
)]
#[archive_attr(derive(CheckBytes))]
pub struct ClipTranslate {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(
    Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Clone, PartialEq,
)]
#[archive_attr(derive(CheckBytes))]
pub enum ClipTranslates {
    Normal(ClipTranslate),
    Keyframe(Vec<ClipTranslate>),
    None,
}

impl ClipTranslates {
    pub fn get_translate_at(&self) -> Option<ClipTranslate> {
        match self {
            ClipTranslates::Normal(t) => Some(t.clone()),
            ClipTranslates::Keyframe(_) => todo!(),
            ClipTranslates::None => None,
        }
    }
}
