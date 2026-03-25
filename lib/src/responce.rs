use std::sync::OnceLock;

use rkyv::{Archive, Deserialize, Serialize};

use crate::{PROJECT, SEND_CALLBACK, project::Project};

#[derive(Archive, Deserialize, Serialize)]
#[repr(u8)]
pub enum Response {
    Test,
    ProjectAll { project: Project },
}

#[repr(C)]
pub struct ResponseCallbacks {
    pub on_test: extern "C" fn(),
}

static RESPONCE_CALLBACKS: OnceLock<ResponseCallbacks> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "C" fn set_responce_callbacks(callbacks: ResponseCallbacks) {
    RESPONCE_CALLBACKS.set(callbacks).ok();
}

#[unsafe(no_mangle)]
pub extern "C" fn on_responce_recveve(ptr: *const u8, len: usize) {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let archived = unsafe { rkyv::archived_root::<Response>(&bytes) };
    match archived {
        ArchivedResponse::Test => {
            if let Some(cb) = RESPONCE_CALLBACKS.get() {
                ((cb.on_test)());
            }
        }
        ArchivedResponse::ProjectAll { project } => {
            let real_project: Project = project.deserialize(&mut rkyv::Infallible).unwrap();
            *PROJECT.write().unwrap() = Some(real_project);
        }
    }
}

fn send_response(response: Response) {
    let bytes: rkyv::AlignedVec = rkyv::to_bytes::<_, 1024>(&response).unwrap();
    if let Some(send_cb) = SEND_CALLBACK.get() {
        send_cb(bytes.as_ptr(), bytes.len());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn res_test() {
    let cmd = Response::Test;
    send_response(cmd);
}

#[unsafe(no_mangle)]
pub extern "C" fn res_project_all() {
    let lock = PROJECT.read().unwrap();

    let Some(project) = lock.as_ref() else {
        return;
    };

    let cmd = Response::ProjectAll {
        project: (*project).clone(),
    };
    send_response(cmd);
}
