use std::ffi::c_void;

use nomyoedit_lib::render::{surfacetarget::get_surface_target, wgpuutil::WGpuUtil};

#[unsafe(no_mangle)]
pub extern "C" fn wgpuutil_init_surface(
    window_ptr: *mut c_void,
    display_ptr: *mut c_void,
    width: u32,
    height: u32,
    is_wayland: bool,
) -> *mut WGpuUtil {
    log::debug!("wgpu initing");
    log::debug!("window_ptr: {:x}", window_ptr as usize);
    log::debug!("display_ptr: {:x}", display_ptr as usize);
    log::debug!("width: {}, height: {}", width, height);
    log::debug!("is_wayland: {}", is_wayland);

    let surface = get_surface_target(window_ptr, display_ptr, is_wayland);

    let wpguutil = WGpuUtil::new(surface, width, height);
    Box::into_raw(Box::new(wpguutil))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_drop(ptr: *mut WGpuUtil) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_update_surface(
    ptr: *mut WGpuUtil,
    window_ptr: *mut c_void,
    display_ptr: *mut c_void,
    is_wayland: bool,
) {
    if ptr.is_null() {
        return;
    }
    let wgpuutil = unsafe { &mut (*ptr) };

    let surface = get_surface_target(window_ptr, display_ptr, is_wayland);

    wgpuutil.update_surface(surface);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_update_size(ptr: *mut WGpuUtil, width: u32, height: u32) {
    if ptr.is_null() {
        return;
    }

    let wgpuutil = unsafe { &mut (*ptr) };

    wgpuutil.config.width = width;
    wgpuutil.config.height = height;
    wgpuutil
        .surface
        .configure(&wgpuutil.device, &wgpuutil.config);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_frame(ptr: *mut WGpuUtil) {
    if !ptr.is_null() {
        let result = unsafe { nomyoedit_lib::render::render_frame(&mut (*ptr)) };
        if let Err(err) = result {
            log::error!("{}", err);
        }
    }
}
