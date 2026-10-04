use esotereel_core::server_network_start;
use esotereel_lib::dirs::Directories;
use esotereel_lib::{prebootstrap::PreBootstrapSettings, util::logger::init_logger_with_settings};
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let std_plugin_dir = env::var("ESOTEREEL_PLUGIN_DIR")
        .ok()
        .map(|s| PathBuf::from(s));

    let working_dir = env::var("ESOTEREEL_WORKING_DIR")
        .ok()
        .map(|s| PathBuf::from(s));

    let dirs_def = Directories::new(std_plugin_dir, working_dir);
    let prebootstrap_settings = dirs_def
        .prebootstrap_settings_path()
        .and_then(|path| PreBootstrapSettings::load(&path))
        .unwrap_or_else(|error| {
            eprintln!("Failed to load prebootstrap settings: {error}");
            PreBootstrapSettings::default()
        });

    init_logger_with_settings(log_out_callback, &prebootstrap_settings);

    server_network_start("0.0.0.0:12345", None::<fn(bool, &str)>, dirs_def, None).await;
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
