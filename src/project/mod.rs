use crate::{project::timeline::Timeline, ui::timeline::TimelineType};

pub mod clip;
pub mod layer;
pub mod timeline;

pub struct Project {
    pub timeline: Timeline,
    pub timeline_temp: Timeline,
    pub playhead: i64,
}

impl Project {
    pub(crate) fn new() -> Self {
        Self {
            playhead: 0,
            timeline: Timeline::new(),
            timeline_temp: Timeline::new(),
        }
    }

    pub fn get_timeline_by<'a>(&'a mut self, ttype: TimelineType) -> &'a mut Timeline {
        match ttype {
            TimelineType::MAIN => &mut self.timeline,
            TimelineType::TEMP => &mut self.timeline_temp,
        }
    }
}
