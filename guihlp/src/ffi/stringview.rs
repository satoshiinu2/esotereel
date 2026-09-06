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

    pub fn as_str(&self) -> Option<&str> {
        if self.ptr.is_null() {
            return None;
        }
        let slice = unsafe { slice_from_ptr_or_empty(self.ptr, self.len) };
        std::str::from_utf8(slice).ok()
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

impl From<StringView> for String {
    fn from(value: StringView) -> Self
    where
        Self: Sized,
    {
        StringView::as_string_lossy(&value).into_owned()
    }
}

impl From<String> for StringView {
    fn from(value: String) -> Self {
        StringView::from_str(value.as_str())
    }
}

impl From<&str> for StringView {
    fn from(value: &str) -> Self {
        StringView::from_str(value)
    }
}
