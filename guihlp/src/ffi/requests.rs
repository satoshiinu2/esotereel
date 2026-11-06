use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    project::{TimelineTick, command::CommandRequest, ids::TimelineId},
    render::video::request::request_stream_packets_for_time,
    requests::Request,
    util::result::{EsotereelError, LockExt},
};

use crate::ffi::{result::FfiResultVoid, state::ClientStateHandle, stringview::FfiStringView};

#[unsafe(no_mangle)]
pub extern "C" fn req_test(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
        let network = &state.network;

        network.send(&Request::Test);
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn req_new_project(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
        let network = &state.network;

        network.send(&Request::NewProject);
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_fetch_frame(
    ptr_state: *const ClientStateHandle,
    timeline_id: TimelineId, // ポインタではなくIDで受け取る
    current_frame: TimelineTick,
    visible_range_start: TimelineTick,
    visible_range_end: TimelineTick,
) -> FfiResultVoid {
    // Keep the C ABI parameter while media requests are driven by the fetch cache.
    let _ = current_frame;

    fn inner(
        ptr_state: *const ClientStateHandle,
        timeline_id: TimelineId,
        visible_range_start: TimelineTick,
        visible_range_end: TimelineTick,
    ) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);

        let state_guard = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

        let project_guard = state_guard.project.read_or_err()?;

        let project = project_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;

        let req = {
            project
                .timeline_ref(timeline_id)
                .ok_or_else(|| anyhow::anyhow!("invalid timeline id"))?;

            request_stream_packets_for_time(
                &state_guard.stream_state_map,
                &state_guard.stream_players,
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
    }

    match catch_unwind(AssertUnwindSafe(|| {
        inner(
            ptr_state,
            timeline_id,
            visible_range_start,
            visible_range_end,
        )
    })) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_project_log(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state_guard = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
        let network = &state_guard.network;

        network.send(&Request::DebugFetchProjectStruct);
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn req_load_stream(
    ptr_state: *const ClientStateHandle,
    path: FfiStringView,
) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle, path: FfiStringView) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
        let network = &state.network;

        let path = path.as_str()?.to_string();

        network.send(&Request::InitStream { path });
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, path))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}
