use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub enum SubWindowEvent {
    OpenProject,
    ClipDropped,
    PlayheadMoved(i64),
    RequestRepaint,
}

pub type SubWindowEventQueue = Arc<Mutex<VecDeque<SubWindowEvent>>>;
