use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    project::{camera::CameraInfo, timeline::Timeline},
    render::{
        surfacetarget::{NativeWindowHandle, get_surface_target},
        video::request::request_stream_packets_for_time,
        wgpuutil::WGpuUtil,
    },
};

use crate::{WrapperErrorCode, network::ClientNetworkHandler, wrapper::log_if_panicked};

#[unsafe(no_mangle)]
pub extern "C" fn wgpuutil_init_surface(
    handle: NativeWindowHandle,
    width: u32,
    height: u32,
    out: *mut *mut WGpuUtil,
) -> WrapperErrorCode {
    log::debug!("wgpu initing");
    log::debug!("kind: {:?}", handle.kind);
    log::debug!("window_ptr: {:x}", handle.window_ptr as usize);
    log::debug!("display_ptr: {:x}", handle.display_ptr as usize);
    log::debug!("width: {}, height: {}", width, height);

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let surface = get_surface_target(handle)?;

        let wpguutil = WGpuUtil::new(surface, width, height);
        unsafe { *out = Box::into_raw(Box::new(wpguutil)) }
        Ok(())
    }));

    // catch_unwind自体のパニックと、get_surface_targetが返すResult::Errの
    // 両方をここで一本化してStringViewに落とす
    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(err)) => {
            log::error!("wgpuutil_init_surface: {}", err);
            WrapperErrorCode::error(Some(&err))
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "wgpuutil_init_surface");
            WrapperErrorCode::error(msg.as_deref())
        }
    }
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
    WrapperErrorCode::error(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_detach_surface(ptr: *mut WGpuUtil) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let wgpuutil = unsafe { &mut *ptr };
        wgpuutil.detach_surface(); // surfaceフィールドをNoneにするだけ。Box自体は無傷
    }));
    let msg = log_if_panicked(result, "wgpuutil_drop");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_attach_surface(
    ptr: *mut WGpuUtil,
    handle: NativeWindowHandle,
) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let wgpuutil = unsafe { &mut (*ptr) };

        let surface = get_surface_target(handle)?;
        wgpuutil.attach_surface(surface);
        Ok(())
    }));

    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(err)) => {
            log::error!("wgpuutil_update_surface: {}", err);
            WrapperErrorCode::error(Some(&err))
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "wgpuutil_update_surface");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_update_size(
    ptr: *mut WGpuUtil,
    width: u32,
    height: u32,
) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let wgpuutil = unsafe { &mut (*ptr) };

        wgpuutil.config.width = width;
        wgpuutil.config.height = height;
        if let Some(surface) = &wgpuutil.surface {
            surface.configure(&wgpuutil.device, &wgpuutil.config);
        }
    }));

    let msg = log_if_panicked(result, "wgpuutil_update_size");
    WrapperErrorCode::error_from_option(msg.as_deref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_render_frame(
    ptr_wgpu: *mut WGpuUtil,
    ptr_network: *const ClientNetworkHandler,
    ptr_timeline: *const Timeline,
    ptr_camera_info: *const CameraInfo,
    current_frame: i64,
) -> WrapperErrorCode {
    if ptr_wgpu.is_null()
        || ptr_network.is_null()
        || ptr_timeline.is_null()
        || ptr_camera_info.is_null()
    {
        return WrapperErrorCode::null_ptr();
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let network = unsafe { &*ptr_network };
        let app_state = network.app_state.lock().expect("mutex poisoned");
        let timeline = unsafe { &*ptr_timeline };
        let wgpuutil = unsafe { &mut *ptr_wgpu };
        let camera_info = unsafe { &*ptr_camera_info };

        // 現在のフレームから2秒先（60fps想定で120フレームなど）を
        // バッファリング対象としてリクエスト関数に渡す
        let lookahead = 60;
        let frame_range = current_frame..current_frame + lookahead;
        let req = request_stream_packets_for_time(timeline, &app_state, frame_range);
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
    WrapperErrorCode::error_from_option(msg.as_deref())
}
