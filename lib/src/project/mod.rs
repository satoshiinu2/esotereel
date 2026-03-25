use crate::project::{clip::Clip, timeline::Timeline};
use rkyv::{Archive, Deserialize, Serialize};

pub mod clip;
pub mod layer;
pub mod timeline;

#[derive(Archive, Deserialize, Serialize, Clone)]
pub struct Project {
    pub timelines: [Timeline; 2],
    pub next_clip_id: u32,
}

impl Project {
    pub(crate) fn new() -> Self {
        Self {
            next_clip_id: 0,
            timelines: [Timeline::new(), Timeline::new()],
        }
    }

    pub fn get_timeline<'a>(&'a mut self, id: usize) -> &'a mut Timeline {
        self.timelines.get_mut(id).unwrap()
    }

    fn new_clip_id(&mut self) -> u32 {
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_timeline(ptr: *const Project, idx: usize) -> *const Timeline {
    unsafe { &(*ptr).timelines[idx] }
}
