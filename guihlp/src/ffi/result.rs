use std::any::Any;

use esotereel_lib::util::result::format_any_error;

use crate::ffi::stringview::OwnedString;

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

    pub fn err(msg: Box<dyn Any + Send>) -> Self {
        let str = OwnedString::from_string(format_any_error(msg));

        Self {
            is_ok: false,
            value: FfiResultUnion { err: str },
        }
    }

    pub fn from_result(r: Result<T, Box<dyn Any + Send>>) -> Self {
        match r {
            Ok(v) => Self::ok(v),
            Err(e) => Self::err(e),
        }
    }
}
