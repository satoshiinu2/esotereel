use std::sync::OnceLock;

pub mod command;
pub mod project;
pub mod responce;
pub mod types;

pub type OnSendFn = extern "C" fn(*const u8, usize);

pub(crate) static SEND_CALLBACK: OnceLock<OnSendFn> = OnceLock::new();

pub fn set_send_callback(callback: OnSendFn) {
    SEND_CALLBACK.set(callback).ok();
}
