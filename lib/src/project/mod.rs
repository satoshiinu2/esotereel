use crate::project::{clip::Clip, timeline::Timeline};
use rkyv::{Archive, Deserialize, Serialize};

pub mod clip;
pub mod layer;
pub mod timeline;

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct Project {
    timelines: [Timeline; 2],
    next_clip_id: u64,
}

impl Project {
    pub fn new() -> Self {
        Self {
            next_clip_id: 0,
            timelines: [Timeline::new(), Timeline::new()],
        }
    }

    pub fn get_timeline<'a>(&self, id: usize) -> Result<&Timeline, &str> {
        self.timelines.get(id).ok_or("invalid timeline")
    }
    pub fn get_timeline_mut<'a>(&mut self, id: usize) -> Result<&mut Timeline, &str> {
        self.timelines.get_mut(id).ok_or("invalid timeline")
    }
    pub fn get_timeline_count(&self) -> usize {
        self.timelines.len()
    }

    fn new_clip_id(&mut self) -> u64 {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        id
    }
    pub fn new_clip(&mut self, position: i64, duration: i64) -> Clip {
        Clip {
            id: self.new_clip_id(),
            position,
            duration,
        }
    }
}
