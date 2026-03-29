use std::sync::OnceLock;

use rkyv::{Archive, Deserialize, Serialize};

use crate::{SEND_CALLBACK, types::ClipMoveCtx};

type OnReceiveCommandFn = fn(&ArchivedCommand) -> Result<(), String>;

#[derive(Archive, Deserialize, Serialize)]
#[repr(u8)]
pub enum Command {
    Test,
    NewProject,
    ClipsMove {
        timeline_idx: usize,
        clips: Vec<ClipMoveCtx>,
    },
}

pub struct CommandCallbacks {
    pub on_command_recveve: fn(&ArchivedCommand) -> Result<(), String>,
}

static COMMAND_CALLBACK: OnceLock<OnReceiveCommandFn> = OnceLock::new();

pub fn set_command_callbacks(callback: OnReceiveCommandFn) {
    COMMAND_CALLBACK.set(callback).ok();
}

pub fn parse_command(ptr: *const u8, len: usize) {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let archived_cmd = unsafe { rkyv::archived_root::<Command>(&bytes) };

    if let Some(on_command_recveve) = COMMAND_CALLBACK.get() {
        if let Err(msg) = on_command_recveve(archived_cmd) {
            println!("{}", msg);
        }
    }
}

pub fn send_command(command: Command) {
    let bytes = rkyv::to_bytes::<_, 1024>(&command).unwrap();
    if let Some(send_cb) = SEND_CALLBACK.get() {
        send_cb(bytes.as_ptr(), bytes.len());
    }
}
