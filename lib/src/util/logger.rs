use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock, RwLock},
};

use log::{LevelFilter, Log, Metadata, Record};

use crate::prebootstrap::PreBootstrapSettings;

pub type OutLogFn = fn(level: usize, msg: String);

pub(crate) static LOG_CALLBACK: OnceLock<OutLogFn> = OnceLock::new();

struct QtLogger {
    default_level: RwLock<LevelFilter>,
    filters: RwLock<HashMap<String, LevelFilter>>,
}

static LOGGER: LazyLock<QtLogger> = LazyLock::new(|| QtLogger {
    default_level: RwLock::new(LevelFilter::Info),
    filters: RwLock::new(HashMap::new()),
});

impl Log for QtLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let target_filter = self.filters.read().unwrap().get(metadata.target()).copied();
        metadata.level() <= target_filter.unwrap_or(*self.default_level.read().unwrap())
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = record.level() as usize;

            let msg = format!("{}", record.args());
            // ignore Suboptimal present
            if msg.contains("Suboptimal present") {
                return;
            }
            if let Some(log_cb) = LOG_CALLBACK.get() {
                log_cb(level, msg);
            }
        }
    }
    fn flush(&self) {}
}

pub fn init_logger(callback: OutLogFn) {
    init_logger_with_settings(callback, &PreBootstrapSettings::default());
}

pub fn init_logger_with_settings(callback: OutLogFn, settings: &PreBootstrapSettings) {
    LOG_CALLBACK.set(callback).ok();
    *LOGGER.default_level.write().unwrap() = settings.logging.level.as_filter();
    *LOGGER.filters.write().unwrap() = settings
        .logging
        .filters
        .iter()
        .map(|(target, level)| (target.clone(), level.as_filter()))
        .collect();
    log::set_logger(&*LOGGER).unwrap();
    log::set_max_level(LevelFilter::Trace);

    // パニック処理
    std::panic::set_hook(Box::new(|info| {
        // パニックメッセージの取得
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        // 発生場所（ファイル名と行数）の取得
        let location = info
            .location()
            .map(|l| format!(" at {}:{}", l.file(), l.line()))
            .unwrap_or_default();

        let full_msg = format!("PANIC: {}{}", msg, location);

        if let Some(cb) = LOG_CALLBACK.get() {
            cb(1, full_msg);
        }
    }));
}
