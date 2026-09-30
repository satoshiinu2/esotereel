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
        Self::from_vec_with_free_fn(vec, Self::free_vec)
    }

    pub fn from_vec_with_free_fn(
        vec: Vec<T>,
        free_fn: unsafe extern "C" fn(*mut FfiArray<T>),
    ) -> Self {
        let mut v = std::mem::ManuallyDrop::new(vec);
        Self {
            ptr: v.as_mut_ptr(),
            len: v.len(),
            cap: v.capacity(),
            free_fn,
        }
    }

    pub fn from_slice(slice: &[T]) -> Self
    where
        T: Clone,
    {
        let mut v = std::mem::ManuallyDrop::new(slice.to_vec());
        Self {
            ptr: v.as_mut_ptr(),
            len: v.len(),
            cap: v.capacity(),
            free_fn: Self::free_vec,
        }
    }

    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        if self.ptr.is_null() {
            return Vec::new();
        }
        let slice = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        slice.to_vec()
    }

    pub fn as_slice(&self) -> &[T] {
        if self.ptr.is_null() {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Release using the allocator-specific callback carried by this array.
    pub unsafe fn free(&mut self) {
        let free_fn = self.free_fn;
        unsafe {
            free_fn(self);
        }
    }

    unsafe extern "C" fn free_vec(s: *mut FfiArray<T>) {
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
}
