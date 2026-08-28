use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    project::{Timeline, camera::CameraInfo, ids::TimelineId},
    render::wgpuutil::{OffscreenTarget, WGpuUtil},
};

use crate::{WrapperErrorCode, network::ClientNetworkHandler, wrapper::log_if_panicked};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_render_frame_offscreen(
    ptr_wgpu: *mut WGpuUtil,
    ptr_offscreen: *mut OffscreenTarget,
    ptr_network: *const ClientNetworkHandler,
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
        || ptr_network.is_null()
        || ptr_camera_info.is_null()
    {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), WrapperErrorCode> {
        let network = unsafe { &*ptr_network };
        let wgpuutil = unsafe { &mut *ptr_wgpu };
        let offscreen = unsafe { &*ptr_offscreen };
        let camera_info = unsafe { &*ptr_camera_info };

        let app_state = network.app_state.lock().expect("mutex poisoned");

        let project_arc = match app_state.project.as_ref() {
            Some(arc) => Ok(arc),
            None => Err(WrapperErrorCode::not_found(Some("project not found"))),
        }?;

        // Get timeline data with minimal lock time
        let lock = project_arc.read().unwrap();
        let timeline = match lock.timeline(timeline_id) {
            Some(tl) => Ok(tl),
            None => Err(WrapperErrorCode::not_found(Some("timeline not found"))),
        }?;

        esotereel_lib::render::render_frame_offscreen(
            wgpuutil,
            offscreen,
            timeline,
            &app_state,
            camera_info,
            current_frame,
        )
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
