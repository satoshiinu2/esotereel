use esotereel_lib::{OnSendFn, project::Project};

use crate::PROJECT;

pub mod command;
pub mod logger;
pub mod project;
pub mod render;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn parse_responce(ptr: *const u8, len: usize) {
    esotereel_lib::responce::parse_responce(ptr, len);
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_send_callback(callback: OnSendFn) {
    esotereel_lib::set_send_callback(callback);
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
