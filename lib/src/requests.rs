use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::project::commands::Command;

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Request {
    Test,
    NewProject,
    ProjectAll,
    Command {
        command: Command,
        timeline_idx: usize,
    },
    LoadStream {
        path: String,
    },
    FetchStreamData {
        resource_id: u32,
        seek_seconds: f64,
        count: usize,
    },
}
