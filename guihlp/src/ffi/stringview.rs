use esotereel_lib::util::result::EsotereelError;

use crate::slice_from_ptr_or_empty;
use std::borrow::Cow;

#[repr(C)]
pub struct StringView {
    pub ptr: *const u8,
    pub len: usize,
}

impl StringView {
    pub fn from_str(s: &str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    pub fn from_option_str(s: Option<&str>) -> Self {
        if let Some(s) = s {
            Self {
                ptr: s.as_ptr(),
                len: s.len(),
            }
        } else {
            Self::zero()
        }
    }

    pub fn as_str(&self) -> anyhow::Result<&str> {
        if self.ptr.is_null() {
            anyhow::bail!(EsotereelError::NullPointer(
                "StringView pointer is null".to_owned()
            ));
        }
        let slice = unsafe { slice_from_ptr_or_empty(self.ptr, self.len) };
        std::str::from_utf8(slice).map_err(anyhow::Error::from)
    }

    pub fn as_string_lossy(&self) -> Cow<'_, str> {
        if self.ptr.is_null() {
            return Cow::Owned(String::new());
        }
        let slice = unsafe { slice_from_ptr_or_empty(self.ptr, self.len) };
        String::from_utf8_lossy(slice)
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub fn zero() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

/// 手動で解放しないといけない
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OwnedString {
    pub ptr: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl OwnedString {
    pub fn from_string(s: String) -> Self {
        let mut s = s;

        let ptr = s.as_mut_ptr();
        let len = s.len();
        let capacity = s.capacity();

        std::mem::forget(s);

        Self { ptr, len, capacity }
    }

    pub fn as_str(&self) -> anyhow::Result<&str> {
        if self.ptr.is_null() {
            anyhow::bail!(EsotereelError::NullPointer(
                "OwnedString pointer is null".to_owned()
            ));
        }
        let slice = unsafe { slice_from_ptr_or_empty(self.ptr, self.len) };
        std::str::from_utf8(slice).map_err(anyhow::Error::from)
    }

    pub fn as_string_lossy(&self) -> Cow<'_, str> {
        if self.ptr.is_null() {
            return Cow::Owned(String::new());
        }
        let slice = unsafe { slice_from_ptr_or_empty(self.ptr, self.len) };
        String::from_utf8_lossy(slice)
    }

    pub fn zero() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub unsafe fn free(self) {
        if self.ptr.is_null() {
            return;
        }

        let _ = unsafe { String::from_raw_parts(self.ptr, self.len, self.capacity) };
    }
}

/// Frees a string that was allocated by Rust and returned via OwnedString
#[unsafe(no_mangle)]
pub unsafe extern "C" fn owned_string_free(str: OwnedString) {
    unsafe {
        str.free();
    }
}

impl From<StringView> for String {
    fn from(value: StringView) -> Self
    where
        Self: Sized,
    {
        StringView::as_string_lossy(&value).into_owned()
    }
}

impl From<&str> for StringView {
    fn from(value: &str) -> Self {
        StringView::from_str(value)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OwnedStringArray {
    pub ptr: *mut OwnedString,
    pub len: usize,
    pub capacity: usize,
}

impl OwnedStringArray {
    pub fn from_vec(mut values: Vec<OwnedString>) -> Self {
        let result = Self {
            ptr: values.as_mut_ptr(),
            len: values.len(),
            capacity: values.capacity(),
        };

        std::mem::forget(values);

        result
    }

    pub fn zero() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub unsafe fn free(self) {
        if self.ptr.is_null() {
            return;
        }

        let values = unsafe { Vec::from_raw_parts(self.ptr, self.len, self.capacity) };

        for value in values {
            unsafe {
                value.free();
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn owned_string_array_free(value: OwnedString) {
    unsafe {
        value.free();
    }
}
