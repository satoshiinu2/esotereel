#[repr(C)]
pub struct FfiArrayView<T> {
    pub ptr: *mut T,
    pub len: usize,
}
