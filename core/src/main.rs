use esotereel_lib::project::Project;
use esotereel_lib::util::logger::init_logger;

use std::sync::RwLock;

use crate::network::start_poll_packets;

mod network;
mod project;
mod requests;

pub static PROJECT: RwLock<Option<Project>> = RwLock::new(None);
fn main() {
    init_logger(log_out_callback);

    start_poll_packets();
}

fn log_out_callback(level: usize, msg: String) {
    let level_str = match level {
        1 => "ERROR",
        2 => "WARN",
        3 => "INFO",
        4 => "DEBUG",
        5 => "TRACE",
        _ => "LOG",
    };
    println!("[{}] {}", level_str, msg);
}
