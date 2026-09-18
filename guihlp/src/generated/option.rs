// 自動生成ファイル。手動編集禁止。
use super::*;

#[warn(non_camel_case_types)]
pub type FfiOption_i32 = FfiOption<i32>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_option_i32_free(opt: *mut FfiOption<i32>) {
unsafe {
if (*opt).has_value {
std::ptr::drop_in_place(&mut (*opt).value.some);
}
}
}

#[warn(non_camel_case_types)]
pub type FfiOption_CFieldValue = FfiOption<CFieldValue>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_option_CFieldValue_free(opt: *mut FfiOption<CFieldValue>) {
unsafe {
if (*opt).has_value {
std::ptr::drop_in_place(&mut (*opt).value.some);
}
}
}

