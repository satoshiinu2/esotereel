use crate::ffi::field_value::CFieldValue;

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

macro_rules! export_array {
    ($name:ident, $ty:ty) => {
        paste::paste! {
            #[allow(non_camel_case_types)]
            pub type [<FfiArray_ $name>] = FfiArray<$ty>;

            #[unsafe(no_mangle)]
            pub extern "C" fn [<ffi_array_ $name _free>](arr: *mut FfiArray<$ty>) {
                unsafe {
                    let a = &mut *arr;
                    if !a.ptr.is_null() {
                        drop(Vec::from_raw_parts(a.ptr, a.len, a.cap));
                        a.ptr = std::ptr::null_mut();
                        a.len = 0;
                        a.cap = 0;
                    }
                }
            }
        }
    };
}

// export_array!(i32, i32);
// export_array!(CFieldValue, CFieldValue);
