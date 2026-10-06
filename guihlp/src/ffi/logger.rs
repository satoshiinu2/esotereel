use crate::ffi::stringview::FfiStringView;
use crate::logger::{CLogLevel, LogOutCStrFn};

#[unsafe(no_mangle)]
pub extern "C" fn init_rust_logger(callback: LogOutCStrFn) {
    crate::logger::init_rust_logger(callback);
}

#[unsafe(no_mangle)]
pub extern "C" fn set_log_level(target: FfiStringView, level: CLogLevel) {
    if let Ok(target_str) = target.as_str() {
        crate::logger::set_log_level(target_str, level);
    }
}
