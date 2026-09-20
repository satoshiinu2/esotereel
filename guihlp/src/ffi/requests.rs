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
        let state = ClientStateHandle::from_ptr(ptr_state);

        let state_guard = state.lock().expect("mutex poisoned");

        let project_guard = state_guard.project.read().expect("mutex poisoned");

        let project = match project_guard.as_ref() {
            Some(arc) => Ok(arc),
            None => Err(IntoWrapperError::NotFound(Some("project not found".into()))),
        }?;

        let req = {
            let timeline = project
                .timeline_ref(timeline_id)
                .ok_or_else(|| IntoWrapperError::Error(Some("invalid timeline id".into())))?;

            let lookahead = 60;
            let frame_range = current_frame..current_frame + lookahead;

            request_stream_packets_for_time(
                timeline,
                &state_guard.stream_state_map,
                &state_guard.stream_players,
                frame_range,
                &state_guard.media_fetch_cache,
            )
        };

        {
            let network = &state_guard.network;

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
    let state_guard = state.lock().expect("mutex poisoned");
    let network = &state_guard.network;

    network.send(&Request::DebugFetchProjectStruct);
}

#[unsafe(no_mangle)]
pub extern "C" fn req_load_stream(
    ptr_state: *const ClientStateHandle,
    path: StringView,
) -> WrapperErrorCode {
    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");
    let network = &state.network;

    let Ok(path) = path.as_str() else {
        return WrapperErrorCode::invalid_string_error();
    };
    let path = path.to_string();

    let req = Request::InitStream { path };
    network.send(&req);
    WrapperErrorCode::ok()
}
