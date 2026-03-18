use crate::project::clip::Clip;

pub struct Layer {
    pub index: usize,
    pub clips: Vec<Clip>,
    pub name: String,
}
