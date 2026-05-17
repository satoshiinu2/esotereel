pub mod logger;
pub mod order_map;
pub mod result;
pub mod slot_map;
pub mod types;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Padding<const T: usize> {
    _padding: [u8; T],
}

// 0で埋めても安全であることを宣言
unsafe impl<const T: usize> bytemuck::Zeroable for Padding<T> {}
// コピー可能でパディング要件を満たすことを宣言
unsafe impl<const T: usize> bytemuck::Pod for Padding<T> {}

impl<const T: usize> Default for Padding<T> {
    fn default() -> Self {
        Self { _padding: [0u8; T] }
    }
}
