use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::{project::commands::Command, util::slot_map::SlotMapKey};

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Request {
    Test,
    NewProject,
    ProjectAll,
    Command {
        command: Command,
        timeline_map_key: SlotMapKey,
    },
    InitStream {
        path: String,
    },
    FetchStreamData {
        resource_id: u32,
        seek_range_sec: Range<f64>,
    },
}
