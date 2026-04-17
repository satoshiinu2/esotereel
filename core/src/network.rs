use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::RwLock,
};

use esotereel_lib::{
    requests::{parse_and_handle_request, set_request_callbacks},
    set_send_callback,
};

use crate::requests::on_request_recveve;

static CLIENT_STREAM: RwLock<Option<TcpStream>> = RwLock::new(None);

pub(super) fn start_poll_packets() {
    set_request_callbacks(on_request_recveve);
    set_send_callback(on_send);

    let listener = TcpListener::bind("127.0.0.1:12345").unwrap();
    log::info!("Core server started");

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

            if let Err(e) = parse_and_handle_request(buf.as_ptr(), size) {
                eprintln!("[Esotereel Core Error] {:?}", e);
            }
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
