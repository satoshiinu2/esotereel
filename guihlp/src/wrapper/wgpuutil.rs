use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
};

use esotereel_lib::{
    project::{camera::CameraInfo, timeline::Timeline},
    render::{
        surfacetarget::get_surface_target, video::request::request_stream_packets_for_time,
        wgpuutil::WGpuUtil,
    },
};

use crate::{
    network::ClientNetworkHandler,
    wrapper::{log_if_panicked, stringview::StringView},
};

#[unsafe(no_mangle)]
pub extern "C" fn wgpuutil_init_surface(
    window_ptr: *mut c_void,
    display_ptr: *mut c_void,
    width: u32,
    height: u32,
    is_wayland: bool,
    out: *mut *mut WGpuUtil,
) -> StringView {
    log::debug!("wgpu initing");
    log::debug!("window_ptr: {:x}", window_ptr as usize);
    log::debug!("display_ptr: {:x}", display_ptr as usize);
    log::debug!("width: {}, height: {}", width, height);
    log::debug!("is_wayland: {}", is_wayland);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let surface = get_surface_target(window_ptr, display_ptr, is_wayland);

        let wpguutil = WGpuUtil::new(surface, width, height);
        unsafe { *out = Box::into_raw(Box::new(wpguutil)) }
    }));

    let msg = log_if_panicked(result, "render_frame");
    StringView::from_option_str(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_drop(ptr: *mut WGpuUtil) -> StringView {
    if ptr.is_null() {
        return StringView::from_str("nullptr");
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        unsafe { drop(Box::from_raw(ptr)) };
    }));

    let msg = log_if_panicked(result, "render_frame");
    StringView::from_option_str(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_update_surface(
    ptr: *mut WGpuUtil,
    window_ptr: *mut c_void,
    display_ptr: *mut c_void,
    is_wayland: bool,
) -> StringView {
    if ptr.is_null() {
        return StringView::from_str("nullptr");
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let wgpuutil = unsafe { &mut (*ptr) };

        let surface = get_surface_target(window_ptr, display_ptr, is_wayland);

        wgpuutil.update_surface(surface);
    }));

    let msg = log_if_panicked(result, "render_frame");
    StringView::from_option_str(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_update_size(
    ptr: *mut WGpuUtil,
    width: u32,
    height: u32,
) -> StringView {
    if ptr.is_null() {
        return StringView::from_str("nullptr");
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let wgpuutil = unsafe { &mut (*ptr) };

        wgpuutil.config.width = width;
        wgpuutil.config.height = height;
        wgpuutil
            .surface
            .configure(&wgpuutil.device, &wgpuutil.config);
    }));

    let msg = log_if_panicked(result, "render_frame");
    StringView::from_option_str(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_render_frame(
    ptr_wgpu: *mut WGpuUtil,
    ptr_network: *const ClientNetworkHandler,
    ptr_timeline: *const Timeline,
    ptr_camera_info: *const CameraInfo,
    current_frame: i64,
) -> StringView {
    if ptr_wgpu.is_null()
        || ptr_network.is_null()
        || ptr_timeline.is_null()
        || ptr_camera_info.is_null()
    {
        return StringView::from_str("nullptr");
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let network = unsafe { &*ptr_network };
        let app_state = network.app_state.lock().expect("mutex poisoned");
        let timeline = unsafe { &*ptr_timeline };
        let wgpuutil = unsafe { &mut (*ptr_wgpu) };
        let camera_info = unsafe { &*ptr_camera_info };

        let req = request_stream_packets_for_time(timeline, &app_state, current_frame);
        for req in req.iter() {
            network.send(req);
        }

        let render_res = esotereel_lib::render::render_frame(
            wgpuutil,
            timeline,
            &app_state,
            &camera_info,
            current_frame,
        );

        if let Err(err) = render_res {
            log::error!("{}", err);
        }
    }));

    let msg = log_if_panicked(result, "render_frame");
    StringView::from_option_str(msg.as_deref())
}
