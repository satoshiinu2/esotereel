use std::sync::{OnceLock, RwLock};

use crate::project::Project;

pub mod command;
pub mod project;
pub mod responce;

type OnSendFn = extern "C" fn(*const u8, usize);

pub(crate) static SEND_CALLBACK: OnceLock<OnSendFn> = OnceLock::new();
static PROJECT: RwLock<Option<Project>> = RwLock::new(None);

#[unsafe(no_mangle)]
pub extern "C" fn set_send_callbacks(callback: OnSendFn) {
    SEND_CALLBACK.set(callback).ok();
}

#[unsafe(no_mangle)]
pub extern "C" fn get_project() -> *const Project {
    PROJECT
        .read()
        .unwrap()
        .as_ref()
        .map(|p| p as *const Project)
        .unwrap_or(std::ptr::null())
}
