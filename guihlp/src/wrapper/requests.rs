use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    project::{TimelineTick, commands::Command, ids::TimelineId},
    render::video::request::request_stream_packets_for_time,
    requests::Request,
};

use crate::{
    IntoWrapperError, WrapperErrorCode,
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
    timeline_id: TimelineId, // ポインタではなくIDで受け取る
    current_frame: TimelineTick,
    visible_range_start: TimelineTick,
    visible_range_end: TimelineTick,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let network = unsafe { &*ptr_network };

        let project_arc = {
            let app_state = network.app_state.lock().expect("mutex poisoned");
            app_state.project.as_ref().cloned()
        };

        let project_arc = project_arc.ok_or_else(|| IntoWrapperError::NotFound(None))?;

        let req = {
            let app_state = network.app_state.lock().expect("mutex poisoned");
            let project_guard = project_arc
                .read()
                .map_err(|_| IntoWrapperError::Error(Some("lock poisoned".into())))?;
            let timeline = project_guard
                .timeline(timeline_id)
                .ok_or_else(|| IntoWrapperError::Error(Some("invalid timeline id".into())))?;

            let lookahead = 60;
            let frame_range = current_frame..current_frame + lookahead;

            request_stream_packets_for_time(timeline, &app_state, frame_range)
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
