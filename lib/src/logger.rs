use std::sync::OnceLock;

use log::{Log, Metadata, Record};

pub type OutLogFn = fn(level: usize, msg: String);

pub(crate) static LOG_CALLBACK: OnceLock<OutLogFn> = OnceLock::new();

struct QtLogger;

impl Log for QtLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
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
    LOG_CALLBACK.set(callback).ok();
    log::set_logger(&QtLogger).unwrap();
    log::set_max_level(log::LevelFilter::Info);

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
