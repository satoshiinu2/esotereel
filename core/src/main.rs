use nomyoedit_lib::command::set_command_callbacks;
use nomyoedit_lib::command::{ArchivedCommand, parse_command};
use nomyoedit_lib::project::Project;
use nomyoedit_lib::project::clip::Clip;
use nomyoedit_lib::responce::{Response, send_response};
use nomyoedit_lib::set_send_callback;
use nomyoedit_lib::types::ClipMoveCtx;
use rkyv::Deserialize;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::RwLock;

use crate::project::clip_move_mul_core;

mod project;

static PROJECT: RwLock<Option<Project>> = RwLock::new(None);
static CLIENT_STREAM: RwLock<Option<TcpStream>> = RwLock::new(None);
fn main() {
    let listener = TcpListener::bind("127.0.0.1:12345").unwrap();
    println!("Rust server started");

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

fn on_command_recveve(command: &ArchivedCommand) {
    match command {
        ArchivedCommand::Test => {}
        ArchivedCommand::NewProject => {
            *PROJECT.write().unwrap() = Some(Project::new());

            // send updates
            let lock = PROJECT.read().unwrap();

            let Some(project) = lock.as_ref() else {
                return;
            };

            let cmd = Response::ProjectAll {
                project: project.clone(),
            };
            send_response(cmd);
        }
        ArchivedCommand::ClipsMove {
            timeline_type,
            clips,
        } => {
            let timeline_type = *timeline_type as usize;
            if let Some(project) = PROJECT.write().unwrap().as_mut() {
                let moved_clips: Vec<ClipMoveCtx> =
                    clips.deserialize(&mut rkyv::Infallible).unwrap();
                let mut updates: HashMap<usize, Vec<Clip>> = HashMap::new();
                let timeline = project.get_timeline_mut(timeline_type);
                clip_move_mul_core(timeline, moved_clips, &mut updates);

                // send updates
                let cmd = Response::ClipUpdates {
                    timeline_type,
                    updates,
                };
                send_response(cmd);
            }
        }
    }
}
