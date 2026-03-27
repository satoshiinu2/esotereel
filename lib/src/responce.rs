use std::{collections::HashMap, sync::OnceLock};

use rkyv::{Archive, Deserialize, Serialize};

use crate::{
    SEND_CALLBACK,
    project::{Project, clip::Clip},
};

type OnReceiveResponceFn = fn(&ArchivedResponse);

#[derive(Archive, Deserialize, Serialize)]
#[repr(u8)]
pub enum Response {
    Test,
    ProjectAll {
        project: Project,
    },
    ClipUpdates {
        timeline_type: usize,
        updates: HashMap<usize, Vec<Clip>>,
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

pub fn parse_responce(ptr: *const u8, len: usize) {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let archived_resp = unsafe { rkyv::archived_root::<Response>(&bytes) };

    if let Some(on_responce_recveve) = RESPONCE_CALLBACK.get() {
        on_responce_recveve(archived_resp);
    }
}

pub fn send_response(response: Response) {
    let bytes: rkyv::AlignedVec = rkyv::to_bytes::<_, 1024>(&response).unwrap();
    if let Some(send_cb) = SEND_CALLBACK.get() {
        send_cb(bytes.as_ptr(), bytes.len());
    }
}
