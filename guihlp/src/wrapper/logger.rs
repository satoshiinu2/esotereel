use std::sync::OnceLock;

use esotereel_lib::util::logger::init_logger;

pub type LogOutCStrFn = extern "C" fn(level: usize, ptr: *const u8, len: usize);

pub(crate) static LOG_C_CALLBACK: OnceLock<LogOutCStrFn> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "C" fn init_rust_logger(callback: LogOutCStrFn) {
    LOG_C_CALLBACK.set(callback).ok();
    init_logger(log_out_callback_wrapper);
}

fn log_out_callback_wrapper(level: usize, msg: String) {
    if let Some(log_cb) = LOG_C_CALLBACK.get() {
        log_cb(level, msg.as_ptr(), msg.len());
    }
}
