use log::{LevelFilter, Log, Metadata, Record};
use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock, RwLock},
};

pub type LogOutCStrFn = extern "C" fn(level: usize, target: crate::ffi::stringview::FfiStringView, msg: crate::ffi::stringview::FfiStringView);

pub(crate) static LOG_C_CALLBACK: OnceLock<LogOutCStrFn> = OnceLock::new();

#[repr(u8)]
pub enum CLogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<CLogLevel> for LevelFilter {
    fn from(value: CLogLevel) -> Self {
        match value {
            CLogLevel::Off => LevelFilter::Off,
            CLogLevel::Error => LevelFilter::Error,
            CLogLevel::Warn => LevelFilter::Warn,
            CLogLevel::Info => LevelFilter::Info,
            CLogLevel::Debug => LevelFilter::Debug,
            CLogLevel::Trace => LevelFilter::Trace,
        }
    }
}

struct GuiLogger {
    default_level: RwLock<LevelFilter>,
    filters: RwLock<HashMap<String, LevelFilter>>,
}

impl Default for GuiLogger {
    fn default() -> Self {
        Self {
            default_level: RwLock::new(LevelFilter::Info),
            filters: RwLock::new(HashMap::new()),
        }
    }
}

static LOGGER: LazyLock<GuiLogger> = LazyLock::new(GuiLogger::default);

pub fn level_filter_from_name(name: &str) -> Option<LevelFilter> {
    match name.to_ascii_lowercase().as_str() {
        "off" => Some(LevelFilter::Off),
        "error" => Some(LevelFilter::Error),
        "warn" | "warning" => Some(LevelFilter::Warn),
        "info" => Some(LevelFilter::Info),
        "debug" => Some(LevelFilter::Debug),
        "trace" => Some(LevelFilter::Trace),
        _ => None,
    }
}

pub fn set_default_log_level(level: LevelFilter) {
    *LOGGER.default_level.write().unwrap() = level;
}

pub fn replace_log_filters(filters: HashMap<String, LevelFilter>) {
    *LOGGER.filters.write().unwrap() = filters;
}

pub fn init_rust_logger(callback: LogOutCStrFn) {
    LOG_C_CALLBACK.set(callback).ok();
    let _ = log::set_logger(&*LOGGER);
    log::set_max_level(LevelFilter::Trace);
}

pub fn set_log_level(target: &str, level: CLogLevel) {
    LOGGER
        .filters
        .write()
        .unwrap()
        .insert(target.to_string(), level.into());
}

impl Log for GuiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let target_filter = self.filters.read().unwrap().get(metadata.target()).copied();
        let filter = target_filter.unwrap_or(*self.default_level.read().unwrap());

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
