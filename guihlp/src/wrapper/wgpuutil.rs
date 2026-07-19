use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    project::{camera::CameraInfo, timeline::Timeline},
    render::{
        surfacetarget::{NativeWindowHandle, get_surface_target},
        video::request::request_stream_packets_for_time,
        wgpuutil::WGpuUtil,
    },
};

use crate::{
    network::ClientNetworkHandler,
    wrapper::{log_if_panicked, stringview::StringView},
};

#[unsafe(no_mangle)]
pub extern "C" fn wgpuutil_init_surface(
    handle: NativeWindowHandle,
    width: u32,
    height: u32,
    out: *mut *mut WGpuUtil,
) -> StringView {
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
        Ok(Ok(())) => StringView::from_option_str(None),
        Ok(Err(err)) => {
            log::error!("wgpuutil_init_surface: {}", err);
            StringView::from_str(&err)
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "wgpuutil_init_surface");
            StringView::from_option_str(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_drop(ptr: *mut WGpuUtil) -> StringView {
    if ptr.is_null() {
        return StringView::from_str("nullptr");
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        unsafe { drop(Box::from_raw(ptr)) };
    }));

    let msg = log_if_panicked(result, "wgpuutil_drop");
    StringView::from_option_str(msg.as_deref())
}

/// wgpu内部でパニック(バリデーションエラー等)が発生した後のWGpuUtilは、
/// 内部状態(ロック中のデータ構造やGPUリソースの参照カウントなど)が
/// 中途半端に書き換わったまま巻き戻っている可能性があり、
/// 通常のDrop(=wgpuutil_drop)を走らせるとその破損した状態を読みに行って
/// 二次的なクラッシュ(heap-use-after-freeなど)を引き起こしうる。
///
/// そのためパニックをcatchした後は、このwgpuutil_leakで
/// **Dropを一切走らせずに**ポインタだけ手放す(意図的なメモリリーク)。
/// リソースは解放されないままになるが、破損した状態への追撃読み書きよりは
/// 安全側に倒した選択。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_leak(ptr: *mut WGpuUtil) {
    if ptr.is_null() {
        return;
    }

    // Box::from_rawで所有権を取り戻した直後、dropを呼ばずBox::leakで手放す。
    // catch_unwindで包む必要はない(Drop自体を呼ばないのでパニックしようがない)。
    let boxed = unsafe { Box::from_raw(ptr) };
    Box::leak(boxed);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_update_surface(
    ptr: *mut WGpuUtil,
    handle: NativeWindowHandle,
) -> StringView {
    if ptr.is_null() {
        return StringView::from_str("nullptr");
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let wgpuutil = unsafe { &mut (*ptr) };

        let surface = get_surface_target(handle)?;
        wgpuutil.update_surface(surface);
        Ok(())
    }));

    match result {
        Ok(Ok(())) => StringView::from_option_str(None),
        Ok(Err(err)) => {
            log::error!("wgpuutil_update_surface: {}", err);
            StringView::from_str(&err)
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "wgpuutil_update_surface");
            StringView::from_option_str(msg.as_deref())
        }
    }
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

    let msg = log_if_panicked(result, "wgpuutil_update_size");
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
    StringView::from_option_str(msg.as_deref())
}
