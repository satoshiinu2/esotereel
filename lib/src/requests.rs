use std::sync::OnceLock;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck, check_archived_root};

use crate::{
    SEND_CALLBACK,
    project::commands::Command,
    util::error::{EsotereelError, EsotereelResult},
};

type OnReceiveCommandFn = fn(&ArchivedRequest) -> EsotereelResult<()>;

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Request {
    Test,
    NewProject,
    Command {
        command: Command,
        timeline_idx: usize,
    },
}

static REQUEST_CALLBACK: OnceLock<OnReceiveCommandFn> = OnceLock::new();

pub fn set_request_callbacks(callback: OnReceiveCommandFn) {
    REQUEST_CALLBACK.set(callback).ok();
}

pub fn parse_and_handle_request(ptr: *const u8, len: usize) -> EsotereelResult<()> {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };

    // validation
    let archived_cmd = check_archived_root::<Request>(bytes)
        .map_err(|e| EsotereelError::IoError(format!("Invalid data format: {:?}", e)))?;

    // callback
    if let Some(on_request_receive) = REQUEST_CALLBACK.get() {
        on_request_receive(archived_cmd)?;
    }

    Ok(())
}

pub fn send_request(request: Request) {
    let bytes = rkyv::to_bytes::<_, 1024>(&request).unwrap();
    if let Some(send_cb) = SEND_CALLBACK.get() {
        send_cb(bytes.as_ptr(), bytes.len());
    }
}
