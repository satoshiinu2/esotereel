use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
};

use esotereel_lib::{
    project::{camera::CameraInfo, ids::TimelineId},
    render::wgpuutil::{OffscreenTarget, WGpuUtil},
    util::result::EsotereelError,
};

use crate::{
    ffi::{array::FfiArray, result::FfiResult, state::ClientStateHandle},
    render::{render_frame_offscreen_ffi, FrameRenderResult},
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
    FfiResult::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        if ptr_wgpu.is_null()
            || ptr_offscreen.is_null()
            || ptr_state.is_null()
            || ptr_camera_info.is_null()
        {
            return Err(EsotereelError::NullPointer("pointer is null".to_string()).into());
        }

        let raw = ptr_state as *const ClientState;
        let state_arc: Arc<ClientState> = unsafe {
            Arc::increment_strong_count(raw);
            Arc::from_raw(raw)
        };
        let wgpuutil = unsafe { &mut *ptr_wgpu };
        let offscreen = unsafe { &*ptr_offscreen };
        let camera_info = unsafe { &*ptr_camera_info };

        // Phase 1: GPUレンダリングコマンドを記録（高速）。
        // 内部で保持するのは Project の read ロックのみ。
        render_frame_offscreen_ffi(
            wgpuutil,
            offscreen,
            &state_arc,
            camera_info,
            timeline_id,
            current_frame,
        )?;

        // Phase 2: GPU readbackを実行（低速）。
        let bytes = offscreen
            .readback(&wgpuutil.device)
            .map_err(|msg| EsotereelError::RenderError(msg))?;

        Ok(FrameRenderResult {
            data: FfiArray::from_vec(bytes),
            width: offscreen.width,
            height: offscreen.height,
        })
    })))
}
