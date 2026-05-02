use std::sync::OnceLock;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck, check_archived_root};

use crate::{
    NO_CLIENT, SEND_REQUEST_CALLBACK, ServerState,
    project::commands::Command,
    util::result::{EsotereelError, EsotereelResult},
};

type OnReceiveCommandFn =
    fn(&ArchivedRequest, client_id: u32, app_state: &ServerState) -> EsotereelResult<()>;

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Request {
    Test,
    NewProject,
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

static REQUEST_CALLBACK: OnceLock<OnReceiveCommandFn> = OnceLock::new();

pub fn set_request_callbacks(callback: OnReceiveCommandFn) {
    REQUEST_CALLBACK.set(callback).ok();
}

pub fn parse_and_handle_request(
    bytes: &[u8],
    client_id: u32,
    app_state: &ServerState,
) -> EsotereelResult<()> {
    // validation
    let archived_cmd = check_archived_root::<Request>(bytes)
        .map_err(|e| EsotereelError::IoError(format!("Invalid data format: {:?}", e)))?;

    // callback
    if let Some(on_request_receive) = REQUEST_CALLBACK.get() {
        on_request_receive(archived_cmd, client_id, app_state)?;
    }

    Ok(())
}

pub fn send_request(request: Request) {
    let bytes = rkyv::to_bytes::<_, 1024>(&request).unwrap();
    if let Some(send_cb) = SEND_REQUEST_CALLBACK.get() {
        // サーバーに送るときはクライアントIDがない
        send_cb(NO_CLIENT, bytes.as_ptr(), bytes.len());
    }
}
