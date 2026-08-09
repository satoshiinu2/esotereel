use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    project::{Timeline, camera::CameraInfo},
    render::{
        video::request::request_stream_packets_for_time,
        wgpuutil::{OffscreenTarget, WGpuUtil},
    },
};

use crate::{WrapperErrorCode, network::ClientNetworkHandler, wrapper::log_if_panicked};

#[unsafe(no_mangle)]
pub extern "C" fn wgpuutil_new(
    width: u32,
    height: u32,
    out: *mut *mut WGpuUtil,
) -> WrapperErrorCode {
    log::debug!(
        "wgpu init (offscreen), width: {}, height: {}",
        width,
        height
    );

    let result = catch_unwind(AssertUnwindSafe(|| {
        let wgpuutil = WGpuUtil::new(width, height);
        unsafe { *out = Box::into_raw(Box::new(wgpuutil)) }
    }));

    let msg = log_if_panicked(result, "wgpuutil_new");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_drop(ptr: *mut WGpuUtil) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        unsafe { drop(Box::from_raw(ptr)) };
    }));
    let msg = log_if_panicked(result, "wgpuutil_drop");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub extern "C" fn offscreen_target_new(
    ptr_wgpu: *mut WGpuUtil,
    width: u32,
    height: u32,
    out: *mut *mut OffscreenTarget,
) -> WrapperErrorCode {
    if ptr_wgpu.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let util = unsafe { &*ptr_wgpu };
        let target = OffscreenTarget::new(&util.device, util.format, width, height);
        unsafe { *out = Box::into_raw(Box::new(target)) }
    }));
    let msg = log_if_panicked(result, "offscreen_target_new");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn offscreen_target_drop(ptr: *mut OffscreenTarget) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        unsafe { drop(Box::from_raw(ptr)) };
    }));
    let msg = log_if_panicked(result, "offscreen_target_drop");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_render_frame_offscreen(
    ptr_wgpu: *mut WGpuUtil,
    ptr_offscreen: *mut OffscreenTarget,
    ptr_network: *const ClientNetworkHandler,
    ptr_timeline: *const Timeline,
    ptr_camera_info: *const CameraInfo,
    current_frame: i64,
    out_data: *mut *mut u8,
    out_len: *mut usize,
    out_width: *mut u32,
    out_height: *mut u32,
) -> WrapperErrorCode {
    if ptr_wgpu.is_null()
        || ptr_offscreen.is_null()
        || ptr_network.is_null()
        || ptr_timeline.is_null()
        || ptr_camera_info.is_null()
    {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let network = unsafe { &*ptr_network };
        let app_state = network.app_state.lock().expect("mutex poisoned");
        let timeline = unsafe { &*ptr_timeline };
        let wgpuutil = unsafe { &mut *ptr_wgpu };
        let offscreen = unsafe { &*ptr_offscreen };
        let camera_info = unsafe { &*ptr_camera_info };

        let lookahead = 60;
        let frame_range = current_frame..current_frame + lookahead;
        let req = request_stream_packets_for_time(timeline, &app_state, frame_range);
        for req in req.iter() {
            network.send(req);
        }

        esotereel_lib::render::render_frame_offscreen(
            wgpuutil,
            offscreen,
            timeline,
            &app_state,
            camera_info,
            current_frame,
        )?;

        let bytes = offscreen.readback(&wgpuutil.device)?;
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
        Ok(Err(e)) => WrapperErrorCode::error(Some(&e)),
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "wgpuutil_render_frame_offscreen");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_free_buffer(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe { drop(Vec::from_raw_parts(ptr, len, len)) };
    }
}
