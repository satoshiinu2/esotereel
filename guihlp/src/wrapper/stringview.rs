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

    pub fn as_str<'a>(&self) -> Option<&'a str> {
        if self.ptr.is_null() {
            return None;
        }
        let slice = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        std::str::from_utf8(slice).ok()
    }

    pub fn as_string_lossy<'a>(&self) -> String {
        if self.ptr.is_null() {
            return String::new();
        }
        let slice = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        String::from_utf8_lossy(slice).into_owned()
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
        StringView::as_string_lossy(&value)
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
