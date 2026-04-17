use esotereel_lib::{OnSendFn, project::Project};

use crate::{PROJECT, wrapper::stringview::StringView};

pub mod commands;
pub mod logger;
pub mod project;
pub mod render;
pub mod requests;
pub mod stringview;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn parse_responce(ptr: *const u8, len: usize) -> StringView {
    if let Err(e) = esotereel_lib::responces::parse_and_handle_responce(ptr, len) {
        return StringView::from_str(&format!("{:?}", e));
    }
    StringView::zero()
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
