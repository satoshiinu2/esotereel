use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use esotereel_lib::{
    project::{camera::CameraInfo, ids::TimelineId},
    render::{
        RenderContext, render_frame_offscreen,
        wgpuutil::{OffscreenTarget, WGpuUtil},
    },
    util::result::EsotereelError,
};

use crate::{
    ffi::{array::FfiArray, result::FfiResult, state::ClientStateHandle},
    state::ClientState,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameRenderResult {
    width: u32,
    height: u32,
    data: FfiArray<u8>,
}

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

        // Arc::into_raw 由来のポインタから、参照カウントを増やして
        // 独立した Arc クローンを作る（元のポインタは消費しない）
        let raw = ptr_state as *const Mutex<ClientState>;
        let state: Arc<Mutex<ClientState>> = unsafe {
            Arc::increment_strong_count(raw);
            Arc::from_raw(raw)
        };
        let wgpuutil = unsafe { &mut *ptr_wgpu };
        let offscreen = unsafe { &*ptr_offscreen };
        let camera_info = unsafe { &*ptr_camera_info };

        let state_guard = state.lock().expect("mutex poisoned");
        let project_guard = state_guard.project.read().expect("mutex poisoned");

        let project = match project_guard.as_ref() {
            Some(arc) => Ok(arc),
            None => Err(EsotereelError::ProjectNotFound),
        }?;

        let timeline = match project.timeline_ref(timeline_id) {
            Some(tl) => Ok(tl),
            None => Err(EsotereelError::TimelineNotFound(timeline_id)),
        }?;

        let ctx = RenderContext {
            path_to_stream: &state_guard.stream_state_map,
            streams: &state_guard.stream_players,

            media_fetch_cache: &state_guard.media_fetch_cache,
            plugin_loader: &state_guard.plugin_loader,

            timeline,
            camera_info,
            current_frame,
        };

        render_frame_offscreen(wgpuutil, offscreen, &ctx)
            .map_err(|msg| EsotereelError::RenderError(msg))?;

        let bytes = offscreen
            .readback(&wgpuutil.device)
            .map_err(|msg| EsotereelError::RenderError(msg))?;

        Ok(FrameRenderResult {
            data: FfiArray::from_vec(bytes),
            width: offscreen.width,
            height: offscreen.height,
        })
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
