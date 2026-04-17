use std::sync::OnceLock;

pub mod project;
pub mod render;
pub mod requests;
pub mod responces;
pub mod util;

pub type OnSendFn = extern "C" fn(*const u8, usize);

pub(crate) static SEND_CALLBACK: OnceLock<OnSendFn> = OnceLock::new();

pub fn set_send_callback(callback: OnSendFn) {
    SEND_CALLBACK.set(callback).ok();
}

pub const ERROR_NO_PROJECT_LOADED: &'static str = "no project loaded";
