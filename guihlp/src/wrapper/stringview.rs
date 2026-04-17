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
    pub unsafe fn as_str<'a>(&self) -> Option<&'a str> {
        if self.ptr.is_null() {
            return None;
        }
        let slice = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        std::str::from_utf8(slice).ok()
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
