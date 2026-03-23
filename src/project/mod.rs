use crate::{
    project::{clip::Clip, timeline::Timeline},
    ui::timeline::TimelineType,
};

pub mod clip;
pub mod layer;
pub mod timeline;

pub struct Project {
    pub timeline: Timeline,
    pub timeline_temp: Timeline,
    pub next_clip_id: usize,
}

impl Project {
    pub(crate) fn new() -> Self {
        Self {
            next_clip_id: 0,
            timeline: Timeline::new(),
            timeline_temp: Timeline::new(),
        }
    }

    pub fn get_timeline_by<'a>(&'a mut self, ttype: TimelineType) -> &'a mut Timeline {
        match ttype {
            TimelineType::Main => &mut self.timeline,
            TimelineType::Temp => &mut self.timeline_temp,
        }
    }

    fn new_clip_id(&mut self) -> usize {
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
