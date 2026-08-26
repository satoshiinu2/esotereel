use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    project::{Tick, commands::Command},
    render::video::request::request_stream_packets_for_time,
    requests::Request,
};

use crate::{
    WrapperErrorCode,
    network::ClientNetworkHandler,
    wrapper::{log_if_panicked, stringview::StringView},
};

#[unsafe(no_mangle)]
pub extern "C" fn req_test(ptr_network: *const ClientNetworkHandler) {
    let network = unsafe { &*ptr_network };

    let req = Request::Test;
    network.send(&req);
}

#[unsafe(no_mangle)]
pub extern "C" fn req_new_project(ptr_network: *const ClientNetworkHandler) {
    let network = unsafe { &*ptr_network };

    let req = Request::NewProject;
    network.send(&req);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_fetch_frame(
    ptr_network: *const ClientNetworkHandler,
    timeline_id: u64, // ポインタではなくIDで受け取る
    current_frame: Tick,
    visible_range_start: Tick,
    visible_range_end: Tick,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let network = unsafe { &*ptr_network };

        // Single lock acquisition to get all needed data
        let (project_arc, streams, path_to_stream) = {
            let app_state = network.app_state.lock().expect("mutex poisoned");
            let project_arc = app_state
                .project
                .as_ref()
                .ok_or_else(|| "project not loaded".to_string())?
                .clone();
            let streams = app_state.streams.clone();
            let path_to_stream = app_state.path_to_stream.clone();
            (project_arc, streams, path_to_stream)
        };

        let req = {
            let project_guard = project_arc
                .read()
                .map_err(|_| "lock poisoned".to_string())?;
            let timeline = project_guard
                .timeline(timeline_id)
                .ok_or_else(|| "invalid timeline id".to_string())?;

            let lookahead = 60;
            let frame_range = current_frame..current_frame + lookahead;

            let temp_state = esotereel_lib::ClientState {
                project: None,
                streams,
                path_to_stream,
            };

            request_stream_packets_for_time(timeline, &temp_state, frame_range)
        };

        for req in req.iter() {
            network.send(req);
        }

        network.send(&Request::FetchClipsInRange {
            timeline_key: timeline_id,
            range: visible_range_start..visible_range_end,
        });

        Ok(())
    }));

    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(e)) => WrapperErrorCode::error(Some(&e)),
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "req_update_frame");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_project_log(ptr_network: *const ClientNetworkHandler) {
    let network = unsafe { &*ptr_network };

    network.send(&Request::DebugFetchProjectStruct);
}

impl ClientNetworkHandler {
    pub(super) fn req_command(&self, timeline_id: u64, command: Command) {
        let req = Request::Command {
            command,
            timeline_id,
        };

        self.send(&req);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn req_load_stream(
    ptr_network: *const ClientNetworkHandler,
    path: StringView,
) -> WrapperErrorCode {
    let network = unsafe { &*ptr_network };

    let Some(path) = path.as_str() else {
        return WrapperErrorCode::invalid_string_error();
    };
    let path = path.to_string();

    let req = Request::InitStream { path };
    network.send(&req);
    WrapperErrorCode::ok()
}
