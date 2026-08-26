use log::{LevelFilter, Log, Metadata, Record};
use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock, RwLock},
};

use crate::wrapper::stringview::StringView;

pub type LogOutCStrFn = extern "C" fn(level: usize, target: StringView, msg: StringView);

pub(crate) static LOG_C_CALLBACK: OnceLock<LogOutCStrFn> = OnceLock::new();

#[derive(Default)]
struct GuiLogger {
    filters: RwLock<HashMap<String, LevelFilter>>,
}

static LOGGER: LazyLock<GuiLogger> = LazyLock::new(GuiLogger::default);

impl Log for GuiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let filters = self.filters.read().unwrap();

        let filter = filters
            .get(metadata.target())
            .copied()
            .unwrap_or(LevelFilter::Info);

        metadata.level() <= filter
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if let Some(log_cb) = LOG_C_CALLBACK.get() {
            let target = record.target();
            let msg = format!("{}", record.args());

            log_cb(record.level() as usize, target.into(), msg.as_str().into());
        }
    }

    fn flush(&self) {}
}

#[unsafe(no_mangle)]
pub extern "C" fn init_rust_logger(callback: LogOutCStrFn) {
    LOG_C_CALLBACK.set(callback).ok();
    let _ = log::set_logger(&*LOGGER);
    log::set_max_level(LevelFilter::Trace);
}

#[unsafe(no_mangle)]
pub extern "C" fn set_log_level(target: StringView, level: usize) {
    let level = match level {
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        4 => LevelFilter::Debug,
        5 => LevelFilter::Trace,
        _ => LevelFilter::Off,
    };

    LOGGER
        .filters
        .write()
        .unwrap()
        .insert(target.as_string_lossy().to_string(), level);
}
