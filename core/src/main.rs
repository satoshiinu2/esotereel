use esotereel_core::server_network_start;
use esotereel_lib::util::logger::init_logger;

#[tokio::main]
async fn main() {
    init_logger(log_out_callback);

    server_network_start("0.0.0.0:12345").await;
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
