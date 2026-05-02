use log::{LevelFilter, Log, Metadata, Record};
use std::sync::OnceLock;

use crate::wrapper::stringview::StringView;

pub type LogOutCStrFn = extern "C" fn(level: usize, target: StringView, msg: StringView);

pub(crate) static LOG_C_CALLBACK: OnceLock<LogOutCStrFn> = OnceLock::new();

struct GuiLogger;

static LOGGER: GuiLogger = GuiLogger;

impl Log for GuiLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if let Some(log_cb) = LOG_C_CALLBACK.get() {
            let target = record.target();
            let msg = format!("{}", record.args());

            // C側にターゲット情報とメッセージを渡す
            // .as_str() を介することで、msg (String) の所有権をこの関数に留め、
            // コールバックが終わるまでメモリを保護します。
            log_cb(record.level() as usize, target.into(), msg.as_str().into());
        }
    }

    fn flush(&self) {}
}

#[unsafe(no_mangle)]
pub extern "C" fn init_rust_logger(callback: LogOutCStrFn) {
    LOG_C_CALLBACK.set(callback).ok();
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Debug);
}
