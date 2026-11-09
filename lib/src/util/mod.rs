/// ユーティリティモジュールです。
///
/// パディング構造体、カラー型、ロガー、順序マップ、rkyv 用カスタムシリアライザ、
/// スロットマップなど、共通して利用される補助型を定義します。

pub mod color;
pub mod logger;
pub mod order_map;
pub mod result;
pub mod rkyv_with;
pub mod slot_map;

/// `bytemuck` 互換のパディング型です。
///
/// `#[repr(C)]` 構造体でパディングバイトをゼロ埋めし、`bytemuck::Pod` / `Zeroable` を
/// 実装することで GPU バッファへの安全な書き込みを可能にします。
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
