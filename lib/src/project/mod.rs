use crate::{
    project::timeline::Timeline,
    util::error::{EsotereelError, EsotereelResult},
};
use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

pub mod clip;
pub mod clipdata;
pub mod commands;
pub mod layer;
pub mod timeline;
pub mod util;

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Project {
    timelines: [Timeline; 2],
}

impl Project {
    pub fn new() -> Self {
        Self {
            timelines: [Timeline::new(), Timeline::new()],
        }
    }

    pub fn get_timeline<'a>(&self, id: usize) -> EsotereelResult<&Timeline> {
        self.timelines
            .get(id)
            .ok_or(EsotereelError::InvalidTimeline)
    }
    pub fn get_timeline_mut<'a>(&mut self, id: usize) -> EsotereelResult<&mut Timeline> {
        self.timelines
            .get_mut(id)
            .ok_or(EsotereelError::InvalidTimeline)
    }
    pub fn get_timeline_count(&self) -> usize {
        self.timelines.len()
    }
}
