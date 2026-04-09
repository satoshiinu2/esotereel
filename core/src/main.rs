use esotereel_lib::command::parse_command;
use esotereel_lib::command::set_command_callbacks;
use esotereel_lib::logger::init_logger;
use esotereel_lib::project::Project;
use esotereel_lib::set_send_callback;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::RwLock;

use crate::network::on_command_recveve;

mod network;
mod project;

pub static PROJECT: RwLock<Option<Project>> = RwLock::new(None);
static CLIENT_STREAM: RwLock<Option<TcpStream>> = RwLock::new(None);
fn main() {
    init_logger(log_out_callback);

    let listener = TcpListener::bind("127.0.0.1:12345").unwrap();
    log::info!("Rust server started");

    set_command_callbacks(on_command_recveve);
    set_send_callback(on_send);

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let mut read_stream = stream.try_clone().unwrap();
        *CLIENT_STREAM.write().unwrap() = Some(stream);

        loop {
            let mut buf = rkyv::AlignedVec::with_capacity(1024);
            buf.resize(1024, 0);

            let size = read_stream.read(&mut buf).unwrap();
            if size == 0 {
                break;
            }

            parse_command(buf.as_ptr(), size);
            // println!("recv: {:?}", &buf[..size]);
        }
    }
}

extern "C" fn on_send(ptr: *const u8, len: usize) {
    let data = unsafe { std::slice::from_raw_parts(ptr, len) };

    if let Ok(mut guard) = CLIENT_STREAM.write() {
        if let Some(ref mut stream) = *guard {
            stream.write_all(data).unwrap();
        }
    }
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
