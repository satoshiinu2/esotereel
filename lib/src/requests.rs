use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::project::{MediaSec, TimelineTick, command::CommandRequest, ids::TimelineId};

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Request {
    Test,
    NewProject,
    ProjectAll,
    Command {
        command: CommandRequest,
        timeline_id: TimelineId,
    },
    InitStream {
        path: String,
    },
    FetchStreamData {
        resource_id: u32,
        ranges: Vec<Range<MediaSec>>,
    },
    FetchClipsInRange {
        timeline_key: u64,
        range: Range<TimelineTick>,
    },
    DebugFetchProjectStruct,
}
