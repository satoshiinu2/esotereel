use muscedit_lib::command::on_command_recveve;
use muscedit_lib::responce::{ResponseCallbacks, set_responce_callbacks};
use muscedit_lib::set_send_callbacks;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::RwLock;

static CLIENT_STREAM: RwLock<Option<TcpStream>> = RwLock::new(None);
fn main() {
    let listener = TcpListener::bind("127.0.0.1:12345").unwrap();
    println!("Rust server started");

    let callbacks = ResponseCallbacks { on_test };

    set_responce_callbacks(callbacks);
    set_send_callbacks(on_send);

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

            on_command_recveve(buf.as_ptr(), size);
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

extern "C" fn on_test() {}
