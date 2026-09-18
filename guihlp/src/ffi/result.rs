use std::any::Any;

use esotereel_lib::util::result::format_any_error;

use crate::ffi::{field_value::CFieldValue, option::FfiOption, stringview::OwnedString};

#[repr(C)]
pub union FfiResultUnion<T: Copy> {
    pub ok: T,
    pub err: OwnedString,
}

#[repr(C)]
pub struct FfiResult<T: Copy> {
    pub is_ok: bool,
    pub value: FfiResultUnion<T>,
}

impl<T: Copy> FfiResult<T> {
    pub fn ok(val: T) -> Self {
        Self {
            is_ok: true,
            value: FfiResultUnion { ok: val },
        }
    }

    pub fn err(e: anyhow::Error) -> Self {
        // {:#} で原因チェーンも含めて整形する
        let str = OwnedString::from_string(format!("{:#}", e));
        Self {
            is_ok: false,
            value: FfiResultUnion { err: str },
        }
    }

    pub fn err_panic(msg: Box<dyn Any + Send>) -> Self {
        let str = OwnedString::from_string(format_any_error(msg));

        Self {
            is_ok: false,
            value: FfiResultUnion { err: str },
        }
    }

    pub fn from_result(r: anyhow::Result<T>) -> Self {
        match r {
            Ok(v) => Self::ok(v),
            Err(e) => Self::err(e),
        }
    }

    pub fn from_panic_result(r: Result<T, Box<dyn Any + Send>>) -> Self {
        match r {
            Ok(v) => Self::ok(v),
            Err(e) => Self::err_panic(e),
        }
    }
}

#[repr(C)]
pub struct FfiResultVoid {
    pub is_ok: bool,
    pub err: OwnedString,
}

impl FfiResultVoid {
    pub fn ok() -> Self {
        Self {
            is_ok: true,
            err: OwnedString::zero(),
        }
    }

    pub fn err(e: anyhow::Error) -> Self {
        // {:#} で原因チェーンも含めて整形する
        let str = OwnedString::from_string(format!("{:#}", e));
        Self {
            is_ok: false,
            err: str,
        }
    }

    pub fn err_panic(msg: Box<dyn Any + Send>) -> Self {
        let str = OwnedString::from_string(format_any_error(msg));

        Self {
            is_ok: false,
            err: str,
        }
    }

    pub fn from_result(r: anyhow::Result<()>) -> Self {
        match r {
            Ok(_) => Self::ok(),
            Err(e) => Self::err(e),
        }
    }

    pub fn from_panic_result(r: Result<(), Box<dyn Any + Send>>) -> Self {
        match r {
            Ok(_) => Self::ok(),
            Err(e) => Self::err_panic(e),
        }
    }
}

macro_rules! export_result {
    ($name:ident, $ty:ty) => {
        paste::paste! {
            #[allow(non_camel_case_types)] // 区切ったほうが良いと思う
            pub type [<FfiResult_ $name>] = FfiResult<$ty>;

            #[unsafe(no_mangle)]
            pub extern "C" fn [<ffi_result_ $name _free_err>](result: *mut FfiResult<$ty>) {
                unsafe {
                    if !(*result).is_ok {
                        std::ptr::drop_in_place(&mut (*result).value.err);
                    }
                }
            }
        }
    };
}
macro_rules! export_option_result {
    ($name:ident, $ty:ty) => {
        paste::paste! {
            #[allow(non_camel_case_types)]
            pub type [<FfiResult_Option_ $name>] = FfiResult<FfiOption<$ty>>;

            #[unsafe(no_mangle)]
            pub extern "C" fn [<ffi_result_option_ $name _free_err>](
                result: *mut FfiResult<FfiOption<$ty>>
            ) {
                unsafe {
                    if !(*result).is_ok {
                        std::ptr::drop_in_place(&mut (*result).value.err);
                    }
                }
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn [<ffi_result_option_ $name _free_opt>](opt: *mut FfiOption<$ty>) {
                unsafe {
                    if (*opt).has_value {
                        std::ptr::drop_in_place(&mut (*opt).value.some);
                    }
                }
            }
        }
    };
}

// export_result!(void, ());
// export_result!(i32, i32);
// export_result!(f32, f32);
// export_result!(bool, bool);
// export_result!(CFieldValue, CFieldValue);
// export_option_result!(CFieldValue, CFieldValue);

// 生成物が使う型
use esotereel_lib::project::ids::ClipId;
use esotereel_lib::project::ids::LayerId;
use esotereel_lib::project::ids::TimelineId;
