use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use esotereel_lib::{
    project::{camera::CameraInfo, ids::TimelineId},
    render::wgpuutil::{OffscreenTarget, WGpuUtil},
    util::result::EsotereelError,
};

use crate::{
    ffi::{result::FfiResult, state::ClientStateHandle},
    render::FrameRenderResult,
    state::ClientState,
};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_render_frame_offscreen(
    ptr_wgpu: *mut WGpuUtil,
    ptr_offscreen: *mut OffscreenTarget,
    ptr_state: *const ClientStateHandle,
    ptr_camera_info: *const CameraInfo,
    timeline_id: TimelineId,
    current_frame: i64,
) -> FfiResult<FrameRenderResult> {
    fn inner(
        ptr_wgpu: *mut WGpuUtil,
        ptr_offscreen: *mut OffscreenTarget,
        ptr_state: *const ClientStateHandle,
        ptr_camera_info: *const CameraInfo,
        timeline_id: TimelineId,
        current_frame: i64,
    ) -> anyhow::Result<FrameRenderResult> {
        if ptr_wgpu.is_null()
            || ptr_offscreen.is_null()
            || ptr_state.is_null()
            || ptr_camera_info.is_null()
        {
            return Err(EsotereelError::NullPointer("pointer is null".to_string()).into());
        }

        let raw = ptr_state as *const Mutex<ClientState>;
        let state: Arc<Mutex<ClientState>> = unsafe {
            Arc::increment_strong_count(raw);
            Arc::from_raw(raw)
        };
        let wgpuutil = unsafe { &mut *ptr_wgpu };
        let offscreen = unsafe { &*ptr_offscreen };
        let camera_info = unsafe { &*ptr_camera_info };

        let state_guard = state.lock().expect("mutex poisoned");

        crate::render::render_frame_offscreen_ffi(
            wgpuutil,
            offscreen,
            &state_guard,
            camera_info,
            timeline_id,
            current_frame,
        )
    }

    FfiResult::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(
            ptr_wgpu,
            ptr_offscreen,
            ptr_state,
            ptr_camera_info,
            timeline_id,
            current_frame,
        )
    })))
}
