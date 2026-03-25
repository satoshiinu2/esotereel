use std::sync::OnceLock;

use rkyv::{Archive, Deserialize, Serialize};

use crate::{PROJECT, SEND_CALLBACK, project::Project, responce::res_project_all};

#[derive(Archive, Deserialize, Serialize)]
#[repr(u8)]
pub enum Command {
    Test,
    NewProject,
}

#[repr(C)]
pub struct CommandCallbacks {
    pub on_test: extern "C" fn(),
}

static COMMAND_CALLBACKS: OnceLock<CommandCallbacks> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "C" fn set_command_callbacks(callbacks: CommandCallbacks) {
    COMMAND_CALLBACKS.set(callbacks).ok();
}

#[unsafe(no_mangle)]
pub extern "C" fn on_command_recveve(ptr: *const u8, len: usize) {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let archived = unsafe { rkyv::archived_root::<Command>(&bytes) };

    match archived {
        ArchivedCommand::Test => {
            if let Some(cb) = COMMAND_CALLBACKS.get() {
                ((cb.on_test)());
            }
        }
        ArchivedCommand::NewProject => {
            *PROJECT.write().unwrap() = Some(Project::new());
            res_project_all();
        }
    }
}

fn send_command(command: Command) {
    let bytes = rkyv::to_bytes::<_, 1024>(&command).unwrap();
    if let Some(send_cb) = SEND_CALLBACK.get() {
        send_cb(bytes.as_ptr(), bytes.len());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cmd_test() {
    let cmd = Command::Test;
    send_command(cmd);
}

#[unsafe(no_mangle)]
pub extern "C" fn cmd_new_project() {
    let cmd = Command::NewProject;
    send_command(cmd);
}
