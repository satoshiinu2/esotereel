use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck, check_archived_root};
use std::sync::OnceLock;

use crate::{
    ClientState, SEND_RESPONSE_CALLBACK,
    project::{ClipUpdateMap, Project},
    util::result::{EsotereelError, EsotereelResult},
};

type OnReceiveResponceFn = fn(&ArchivedResponse, app_state: &ClientState) -> EsotereelResult<()>;

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Response {
    Test,
    ProjectAll {
        project: Project,
    },
    ClipUpdates {
        timeline_type: usize,
        updates: ClipUpdateMap,
    },
    StreamMetadata {
        resource_id: u32,
        codec_id: u16,
        width: u32,
        height: u32,
        extradata: Vec<u8>,
    },
    StreamData {
        resource_id: u32,
        data: Vec<u8>,
        pts: Option<i64>,
        dts: Option<i64>,
        is_key: bool,
        discontinuous: bool,
    },
}

#[repr(C)]
pub struct ResponseCallbacks {
    pub on_test: extern "C" fn(),
    pub on_update_timeline: extern "C" fn(timeline_type: usize),
}

static RESPONCE_CALLBACK: OnceLock<OnReceiveResponceFn> = OnceLock::new();

pub fn set_responce_callbacks(callback: OnReceiveResponceFn) {
    RESPONCE_CALLBACK.set(callback).ok();
}

pub fn parse_and_handle_responce(bytes: &[u8], app_state: &ClientState) -> EsotereelResult<()> {
    // validation
    let archived_resp: &ArchivedResponse = check_archived_root::<Response>(bytes)
        .map_err(|e| EsotereelError::IoError(format!("Invalid data format: {:?}", e)))?;

    // callback
    if let Some(on_responce_recveve) = RESPONCE_CALLBACK.get() {
        on_responce_recveve(archived_resp, app_state)?;
    }

    Ok(())
}

pub fn send_response(client_id: u32, response: Response) {
    let bytes: rkyv::AlignedVec = rkyv::to_bytes::<_, 1024>(&response).unwrap();
    if let Some(send_cb) = SEND_RESPONSE_CALLBACK.get() {
        send_cb(client_id, bytes.as_ptr(), bytes.len());
    }
}
