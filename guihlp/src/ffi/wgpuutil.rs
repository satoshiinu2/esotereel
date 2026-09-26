use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    render::wgpuutil::{OffscreenTarget, WGpuUtil},
    util::result::EsotereelError,
};

use crate::ffi::result::{FfiResult, FfiResultVoid};

/// out 出力パラメータを廃止し、生成したポインタ自体を戻り値として返す形に変更。
pub type WGpuUtilNewResult = FfiResult<*mut WGpuUtil>;

#[unsafe(no_mangle)]
pub extern "C" fn wgpuutil_new(width: u32, height: u32) -> WGpuUtilNewResult {
    fn inner(width: u32, height: u32) -> anyhow::Result<*mut WGpuUtil> {
        log::debug!(
            "wgpu init (offscreen), width: {}, height: {}",
            width,
            height
        );

        let wgpuutil = WGpuUtil::new(width, height);
        Ok(Box::into_raw(Box::new(wgpuutil)))
    }

    match catch_unwind(AssertUnwindSafe(|| inner(width, height))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_drop(ptr: *mut WGpuUtil) -> FfiResultVoid {
    fn inner(ptr: *mut WGpuUtil) -> anyhow::Result<()> {
        if ptr.is_null() {
            return Err(EsotereelError::NullPointer("ptr".to_string()).into());
        }
        unsafe { drop(Box::from_raw(ptr)) };
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

/// out 出力パラメータを廃止し、生成したポインタ自体を戻り値として返す形に変更。
pub type OffscreenTargetNewResult = FfiResult<*mut OffscreenTarget>;

#[unsafe(no_mangle)]
pub extern "C" fn offscreen_target_new(
    ptr_wgpu: *mut WGpuUtil,
    width: u32,
    height: u32,
) -> OffscreenTargetNewResult {
    fn inner(
        ptr_wgpu: *mut WGpuUtil,
        width: u32,
        height: u32,
    ) -> anyhow::Result<*mut OffscreenTarget> {
        if ptr_wgpu.is_null() {
            return Err(EsotereelError::NullPointer("ptr_wgpu".to_string()).into());
        }
        let util = unsafe { &*ptr_wgpu };
        let target = OffscreenTarget::new(&util.device, util.format, width, height);
        Ok(Box::into_raw(Box::new(target)))
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_wgpu, width, height))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn offscreen_target_drop(ptr: *mut OffscreenTarget) -> FfiResultVoid {
    fn inner(ptr: *mut OffscreenTarget) -> anyhow::Result<()> {
        if ptr.is_null() {
            return Err(EsotereelError::NullPointer("ptr".to_string()).into());
        }
        unsafe { drop(Box::from_raw(ptr)) };
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

/// バッファ解放。呼び出し元が正しい(ptr, len)ペアを渡す前提の単純な解放処理であり、
/// 失敗しうる操作ではないため FfiResult 化はせず元のまま据え置き。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wgpuutil_free_buffer(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe { drop(Vec::from_raw_parts(ptr, len, len)) };
    }
}
