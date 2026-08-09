pub mod camera;
pub mod clip;
pub mod commands;
pub mod ids;
pub mod model;
pub mod runtime;
pub mod save;
pub mod transform;
pub mod util;

use std::{collections::HashMap, sync::Arc};

pub use {
    clip::Clip,
    runtime::{Project, timeline::Layer, timeline::Timeline},
};

pub type LayerMapKey = u64;
pub type ClipUpdateMap = HashMap<LayerMapKey, Vec<Clip>>;
