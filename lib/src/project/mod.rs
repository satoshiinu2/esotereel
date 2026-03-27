use crate::project::{clip::Clip, timeline::Timeline};
use rkyv::{Archive, Deserialize, Serialize};

pub mod clip;
pub mod layer;
pub mod timeline;

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct Project {
    pub timelines: [Timeline; 2],
    pub next_clip_id: u64,
}

impl Project {
    pub fn new() -> Self {
        Self {
            next_clip_id: 0,
            timelines: [Timeline::new(), Timeline::new()],
        }
    }

    pub fn get_timeline<'a>(&'a self, id: usize) -> &'a Timeline {
        self.timelines.get(id).unwrap()
    }
    pub fn get_timeline_mut<'a>(&'a mut self, id: usize) -> &'a mut Timeline {
        self.timelines.get_mut(id).unwrap()
    }

    fn new_clip_id(&mut self) -> u64 {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        id
    }
    pub fn new_clip(&mut self, position: u64, duration: u64) -> Clip {
        Clip {
            id: self.new_clip_id(),
            position,
            duration,
        }
    }
}
