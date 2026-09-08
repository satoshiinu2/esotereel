use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::project::{
    MediaSec, TimelineTick,
    command::CommandRequest,
    ids::{ResourceId, TimelineId},
};

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
        resource_id: ResourceId,
        ranges: Vec<Range<MediaSec>>,
    },
    FetchClipsInRange {
        timeline_id: TimelineId,
        range: Range<TimelineTick>,
    },
    DebugFetchProjectStruct,
}
