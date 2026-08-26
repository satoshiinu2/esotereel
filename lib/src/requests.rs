use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::project::{commands::Command, ids::TimelineId};

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Request {
    Test,
    NewProject,
    ProjectAll,
    Command {
        command: Command,
        timeline_id: TimelineId,
    },
    InitStream {
        path: String,
    },
    FetchStreamData {
        resource_id: u32,
        seek_range_sec: Range<f64>,
    },
    FetchClipsInRange {
        timeline_key: u64,
        range: Range<i64>,
    },
    DebugFetchProjectStruct,
}
