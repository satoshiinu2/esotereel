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
};

use crate::{
    WrapperErrorCode,
    ffi::{log_if_panicked, state::ClientStateHandle},
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
    out_data: *mut *mut u8,
    out_len: *mut usize,
    out_width: *mut u32,
    out_height: *mut u32,
) -> WrapperErrorCode {
    if ptr_wgpu.is_null()
        || ptr_offscreen.is_null()
        || ptr_state.is_null()
        || ptr_camera_info.is_null()
    {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), WrapperErrorCode> {
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

        let state = state.lock().expect("mutex poisoned");

        let project_arc = match state.project.as_ref() {
            Some(arc) => Ok(arc),
            None => Err(WrapperErrorCode::not_found(Some("project not found"))),
        }?;

        // Get timeline data with minimal lock time
        let lock = project_arc.read().unwrap();
        let timeline = match lock.timeline(timeline_id) {
            Some(tl) => Ok(tl),
            None => Err(WrapperErrorCode::not_found(Some("timeline not found"))),
        }?;

        let ctx = RenderContext {
            path_to_stream: &state.path_to_stream,
            streams: &state.stream_players,
            timeline,
            camera_info,
            current_frame,
        };

        render_frame_offscreen(wgpuutil, offscreen, &ctx)
            .map_err(|msg| WrapperErrorCode::error(Some(&msg)))?;

        let bytes = offscreen
            .readback(&wgpuutil.device)
            .map_err(|msg| WrapperErrorCode::error(Some(&msg)))?;

        let mut boxed = bytes.into_boxed_slice();
        unsafe {
            *out_data = boxed.as_mut_ptr();
            *out_len = boxed.len();
            *out_width = offscreen.width;
            *out_height = offscreen.height;
        }
        std::mem::forget(boxed);
        Ok(())
    }));

    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(e)) => e,
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "wgpuutil_render_frame_offscreen");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}
