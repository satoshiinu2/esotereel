use nomyoedit_lib::{OnSendFn, project::Project};

use crate::PROJECT;

pub mod clip;
pub mod command;
pub mod layer;
pub mod project;
pub mod timeline;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn parse_responce(ptr: *const u8, len: usize) {
    nomyoedit_lib::responce::parse_responce(ptr, len);
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_send_callback(callback: OnSendFn) {
    nomyoedit_lib::set_send_callback(callback);
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
