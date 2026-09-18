// 自動生成ファイル。手動編集禁止。
use super::*;

#[warn(non_camel_case_types)]
pub type FfiArray_i32 = FfiArray<i32>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_array_i32_free(arr: *mut FfiArray<i32>) {
unsafe {
let a = &mut *arr;
if !a.ptr.is_null() {
drop(Vec::from_raw_parts(a.ptr, a.len, a.cap));
a.ptr = std::ptr::null_mut();
a.len = 0;
a.cap = 0;
}
}
}

#[warn(non_camel_case_types)]
pub type FfiArray_f32 = FfiArray<f32>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_array_f32_free(arr: *mut FfiArray<f32>) {
unsafe {
let a = &mut *arr;
if !a.ptr.is_null() {
drop(Vec::from_raw_parts(a.ptr, a.len, a.cap));
a.ptr = std::ptr::null_mut();
a.len = 0;
a.cap = 0;
}
}
}

#[warn(non_camel_case_types)]
pub type FfiArray_CFieldValue = FfiArray<CFieldValue>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_array_CFieldValue_free(arr: *mut FfiArray<CFieldValue>) {
unsafe {
let a = &mut *arr;
if !a.ptr.is_null() {
drop(Vec::from_raw_parts(a.ptr, a.len, a.cap));
a.ptr = std::ptr::null_mut();
a.len = 0;
a.cap = 0;
}
}
}

