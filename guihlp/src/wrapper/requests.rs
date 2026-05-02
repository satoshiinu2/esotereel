use esotereel_lib::{
    project::commands::Command,
    requests::{Request, send_request},
};

use crate::{WrapperErrorCode, wrapper::stringview::StringView};

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

#[unsafe(no_mangle)]
pub extern "C" fn req_load_stream(path: StringView) -> WrapperErrorCode {
    let Some(path) = path.as_str() else {
        return WrapperErrorCode::Error;
    };
    let path = path.to_string();

    let req = Request::LoadStream { path };
    send_request(req);
    WrapperErrorCode::Ok
}
