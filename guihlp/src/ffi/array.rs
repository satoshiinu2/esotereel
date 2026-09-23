#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiArray<T> {
    pub ptr: *mut T,
    pub len: usize,
    pub cap: usize,

    /// free_fnを呼んでcから解放
    pub free_fn: unsafe extern "C" fn(*mut FfiArray<T>),
}

impl<T> From<Vec<T>> for FfiArray<T> {
    fn from(vec: Vec<T>) -> Self {
        FfiArray::from_vec(vec)
    }
}

impl<T> FfiArray<T> {
    pub fn from_vec(vec: Vec<T>) -> Self {
        let mut v = std::mem::ManuallyDrop::new(vec);
        Self {
            ptr: v.as_mut_ptr(),
            len: v.len(),
            cap: v.capacity(),
            free_fn: Self::free,
        }
    }

    unsafe extern "C" fn free(s: *mut FfiArray<T>) {
        if s.is_null() {
            return;
        }

        let s = unsafe { &mut *s };

        if !s.ptr.is_null() {
            unsafe {
                drop(Vec::from_raw_parts(s.ptr, s.len, s.cap));
            }
        }

        s.ptr = std::ptr::null_mut();
        s.len = 0;
        s.cap = 0;
    }

    /// 解放処理内部で使う。C++側からは触らせない。
    unsafe fn into_vec(self) -> Vec<T> {
        unsafe { Vec::from_raw_parts(self.ptr, self.len, self.cap) }
    }
}
