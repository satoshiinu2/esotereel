use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::project::{ClipUpdateMap, Project};

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Response {
    Test,
    ProjectAll {
        project: Project,
    },
    ClipUpdates {
        timeline_type: usize,
        updates: ClipUpdateMap,
    },
    StreamMetadata {
        path: String,
        resource_id: u32,
        codec_id: u16,
        width: u32,
        height: u32,
        time_base: f64,
        extradata: Vec<u8>,
    },
    StreamData {
        resource_id: u32,
        data: Vec<u8>,
        pts: Option<i64>,
        dts: Option<i64>,
        is_key: bool,
        discontinuous: bool,
    },
    StreamDataEnd {
        resource_id: u32,
        fetched_range: Range<f64>,
    },
}
