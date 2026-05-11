use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct ClipTranslate {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
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
