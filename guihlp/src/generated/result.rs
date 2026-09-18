// 自動生成ファイル。手動編集禁止。build.rs / ffi_types.rs を編集してください。
use super::*;

#[warn(non_camel_case_types)]
pub type FfiResult_i32 = FfiResult<i32>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_result_i32_free_err(result: *mut FfiResult<i32>) {
unsafe {
if !(*result).is_ok {
std::ptr::drop_in_place(&mut (*result).value.err);
}
}
}

#[warn(non_camel_case_types)]
pub type FfiResult_f32 = FfiResult<f32>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_result_f32_free_err(result: *mut FfiResult<f32>) {
unsafe {
if !(*result).is_ok {
std::ptr::drop_in_place(&mut (*result).value.err);
}
}
}

#[warn(non_camel_case_types)]
pub type FfiResult_bool = FfiResult<bool>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_result_bool_free_err(result: *mut FfiResult<bool>) {
unsafe {
if !(*result).is_ok {
std::ptr::drop_in_place(&mut (*result).value.err);
}
}
}

#[warn(non_camel_case_types)]
pub type FfiResult_void = FfiResult<()>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_result_void_free_err(result: *mut FfiResult<()>) {
unsafe {
if !(*result).is_ok {
std::ptr::drop_in_place(&mut (*result).value.err);
}
}
}

#[warn(non_camel_case_types)]
pub type FfiResult_CFieldValue = FfiResult<CFieldValue>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_result_CFieldValue_free_err(result: *mut FfiResult<CFieldValue>) {
unsafe {
if !(*result).is_ok {
std::ptr::drop_in_place(&mut (*result).value.err);
}
}
}

#[warn(non_camel_case_types)]
pub type FfiResult_TimelineId = FfiResult<TimelineId>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_result_TimelineId_free_err(result: *mut FfiResult<TimelineId>) {
unsafe {
if !(*result).is_ok {
std::ptr::drop_in_place(&mut (*result).value.err);
}
}
}

#[warn(non_camel_case_types)]
pub type FfiResult_ClipId = FfiResult<ClipId>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_result_ClipId_free_err(result: *mut FfiResult<ClipId>) {
unsafe {
if !(*result).is_ok {
std::ptr::drop_in_place(&mut (*result).value.err);
}
}
}

