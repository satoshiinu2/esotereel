use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck, check_archived_root};
use std::{collections::HashMap, sync::OnceLock};

use crate::{
    SEND_CALLBACK,
    project::{Project, clip::Clip},
    util::error::{EsotereelError, EsotereelResult},
};

type OnReceiveResponceFn = fn(&ArchivedResponse) -> EsotereelResult<()>;

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Response {
    Test,
    ProjectAll {
        project: Project,
    },
    ClipUpdates {
        timeline_type: usize,
        updates: HashMap<u32, Vec<Clip>>,
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

pub fn parse_and_handle_responce(ptr: *const u8, len: usize) -> EsotereelResult<()> {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };

    // validation
    let archived_resp: &ArchivedResponse = check_archived_root::<Response>(bytes)
        .map_err(|e| EsotereelError::IoError(format!("Invalid data format: {:?}", e)))?;

    // callback
    if let Some(on_responce_recveve) = RESPONCE_CALLBACK.get() {
        on_responce_recveve(archived_resp)?;
    }

    Ok(())
}

pub fn send_response(response: Response) {
    let bytes: rkyv::AlignedVec = rkyv::to_bytes::<_, 1024>(&response).unwrap();
    if let Some(send_cb) = SEND_CALLBACK.get() {
        send_cb(bytes.as_ptr(), bytes.len());
    }
}
