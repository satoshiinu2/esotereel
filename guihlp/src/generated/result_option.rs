// 自動生成ファイル。
use super::*;

#[warn(non_camel_case_types)]
pub type FfiResult_Option_CFieldValue = FfiResult<FfiOption<CFieldValue>>;

#[unsafe(no_mangle)]
pub extern "C" fn ffi_result_option_CFieldValue_free_err(result: *mut FfiResult<FfiOption<CFieldValue>>) {
unsafe {
if !(*result).is_ok {
std::ptr::drop_in_place(&mut (*result).value.err);
}
}
}


