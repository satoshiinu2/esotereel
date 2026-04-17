use std::slice;

use esotereel_lib::{
    project::{clipdata::ClipData, commands::Command},
    requests::{Request, send_request},
    util::types::ClipMoveCtx,
};

use crate::PROJECT;

#[unsafe(no_mangle)]
pub extern "C" fn req_test() {
    let req = Request::Test;
    send_request(req);
}

#[unsafe(no_mangle)]
pub extern "C" fn req_new_project() {
    let req = Request::NewProject;
    send_request(req);
}

pub(crate) fn req_command(timeline_idx: usize, command: Command) {
    let req = Request::Command {
        command,
        timeline_idx,
    };

    send_request(req);
}
