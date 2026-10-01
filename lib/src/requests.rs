use std::{collections::VecDeque, ops::Range};

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
        commands: VecDeque<(TimelineId, CommandRequest)>,
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
    ToolbarAction {
        button_id: String,
        func_name: String,
        run_on: Option<TimelineId>,
        timeline_id: TimelineId,
    },
    Undo {
        timeline_id: TimelineId,
    },
    Redo {
        timeline_id: TimelineId,
    },
}
