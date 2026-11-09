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
    ffi::{
        array::FfiArray,
        result::FfiResult,
        state::ClientStateHandle,
    },
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

        let raw = ptr_state as *const Mutex<ClientState>;
        let state_arc: Arc<Mutex<ClientState>> = unsafe {
            Arc::increment_strong_count(raw);
            Arc::from_raw(raw)
        };
        let wgpuutil = unsafe { &mut *ptr_wgpu };
        let offscreen = unsafe { &*ptr_offscreen };
        let camera_info = unsafe { &*ptr_camera_info };

        // Phase 1: ClientStateロックを保持してGPUレンダリングコマンドを記録（高速）
        {
            let state_guard = state_arc
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

            render_frame_offscreen_ffi(
                wgpuutil,
                offscreen,
                &state_guard,
                camera_info,
                timeline_id,
                current_frame,
            )?;
        } // ClientState ロック解放（ブロック終了でガードドロップ）

        // Phase 2: GPU readbackをClientStateロックなしで実行（低速）。
        // ここでブロックしてもGUIスレッドはClientStateロックを取得できるためフリーズしない。
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
