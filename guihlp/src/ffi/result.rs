use std::any::Any;

use esotereel_lib::util::result::format_any_error;

use crate::ffi::{field_value::CFieldValue, option::FfiOption, stringview::FfiOwnedString};

#[repr(C)]
pub union FfiResultUnion<T: Copy> {
    pub ok: T,
    pub err: FfiOwnedString,
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
        let str = FfiOwnedString::from_string(format!("{:#}", e));
        Self {
            is_ok: false,
            value: FfiResultUnion { err: str },
        }
    }

    pub fn err_panic(msg: Box<dyn Any + Send>) -> Self {
        let str = FfiOwnedString::from_string(format_any_error(msg));

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

    pub fn from_panic_result_result(r: Result<anyhow::Result<T>, Box<dyn Any + Send>>) -> Self {
        match r {
            Ok(r) => Self::from_result(r),
            Err(e) => Self::err_panic(e),
        }
    }
}

impl<T: Copy> FfiResult<FfiOption<T>> {
    pub fn from_panic_result_result_option<F>(
        r: Result<anyhow::Result<Option<F>>, Box<dyn Any + Send>>,
        mapper: impl FnOnce(Option<F>) -> FfiOption<T>,
    ) -> Self {
        match r {
            Ok(r) => Self::from_result(r.map(mapper)),
            Err(e) => Self::err_panic(e),
        }
    }
}

#[repr(C)]
pub struct FfiResultVoid {
    pub is_ok: bool,
    pub err: FfiOwnedString,
}

impl FfiResultVoid {
    pub fn ok() -> Self {
        Self {
            is_ok: true,
            err: FfiOwnedString::zero(),
        }
    }

    pub fn err(e: anyhow::Error) -> Self {
        // {:#} で原因チェーンも含めて整形する
        let str = FfiOwnedString::from_string(format!("{:#}", e));
        Self {
            is_ok: false,
            err: str,
        }
    }

    pub fn err_panic(msg: Box<dyn Any + Send>) -> Self {
        let str = FfiOwnedString::from_string(format_any_error(msg));

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

    pub fn from_panic_result_result(r: Result<anyhow::Result<()>, Box<dyn Any + Send>>) -> Self {
        match r {
            Ok(r) => Self::from_result(r),
            Err(e) => Self::err_panic(e),
        }
    }
}
