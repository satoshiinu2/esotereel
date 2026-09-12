use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use esotereel_lib::{
    project::{TimelineTick, command::CommandRequest, ids::TimelineId},
    render::video::request::request_stream_packets_for_time,
    requests::Request,
};

use crate::{
    IntoWrapperError, WrapperErrorCode,
    ffi::{log_if_panicked, state::ClientStateHandle, stringview::StringView},
    network::ClientNetworkHandler,
    state::ClientState,
};

#[unsafe(no_mangle)]
pub extern "C" fn req_test(ptr_state: *const ClientStateHandle) {
    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");
    let network = &state.network;

    let req = Request::Test;
    network.send(&req);
}

#[unsafe(no_mangle)]
pub extern "C" fn req_new_project(ptr_state: *const ClientStateHandle) {
    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");
    let network = &state.network;

    let req = Request::NewProject;
    network.send(&req);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_fetch_frame(
    ptr_state: *const ClientStateHandle,
    timeline_id: TimelineId, // ポインタではなくIDで受け取る
    current_frame: TimelineTick,
    visible_range_start: TimelineTick,
    visible_range_end: TimelineTick,
) -> WrapperErrorCode {
    if ptr_state.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        // Arc::into_raw 由来のポインタから、参照カウントを増やして
        // 独立した Arc クローンを作る（元のポインタは消費しない）
        let raw = ptr_state as *const Mutex<ClientState>;
        let state: Arc<Mutex<ClientState>> = unsafe {
            Arc::increment_strong_count(raw);
            Arc::from_raw(raw)
        };

        let project_arc = {
            let state = state.lock().expect("mutex poisoned");
            state.project.as_ref().map(Arc::clone)
        };

        let project_arc = project_arc.ok_or_else(|| IntoWrapperError::NotFound(None))?;

        let req = {
            let state = state.lock().expect("mutex poisoned");
            let project_guard = project_arc
                .read()
                .map_err(|_| IntoWrapperError::Error(Some("lock poisoned".into())))?;
            let timeline = project_guard
                .timeline(timeline_id)
                .ok_or_else(|| IntoWrapperError::Error(Some("invalid timeline id".into())))?;

            let lookahead = 60;
            let frame_range = current_frame..current_frame + lookahead;

            request_stream_packets_for_time(
                timeline,
                &state.path_to_stream,
                &state.stream_players,
                frame_range,
            )
        };

        {
            let state = state.lock().expect("mutex poisoned");
            let network = &state.network;

            for req in req.iter() {
                network.send(req);
            }

            network.send(&Request::FetchClipsInRange {
                timeline_id,
                range: visible_range_start..visible_range_end,
            });
        }

        Ok(())
    }));

    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(e)) => {
            e.set_last_err_msg();
            e.into()
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "req_update_frame");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_project_log(ptr_state: *const ClientStateHandle) {
    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");
    let network = &state.network;

    network.send(&Request::DebugFetchProjectStruct);
}

impl ClientNetworkHandler {
    pub(super) fn req_command(&self, timeline_id: u64, command: CommandRequest) {
        let req = Request::Command {
            command,
            timeline_id,
        };

        self.send(&req);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn req_load_stream(
    ptr_state: *const ClientStateHandle,
    path: StringView,
) -> WrapperErrorCode {
    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");
    let network = &state.network;

    let Some(path) = path.as_str() else {
        return WrapperErrorCode::invalid_string_error();
    };
    let path = path.to_string();

    let req = Request::InitStream { path };
    network.send(&req);
    WrapperErrorCode::ok()
}
