use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use crate::util::Padding;

/// 画面サイズとビュープロジェクション行列を GPU に渡すユニフォームです。
///
/// `bytemuck` の `Pod` / `Zeroable` を実装し、`#[repr(C)]` により C 互換のメモリレイアウトを
/// 保証して、wgpu のバッファに直接書き込めます。
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ScreenUniform {
    /// 画面サイズ [width, height]
    pub size: [f32; 2],
    _padding: Padding<8>,
    /// ビュー × プロジェクション行列
    pub view_projection: Mat4,
}

impl ScreenUniform {
    pub fn new(size: [f32; 2], view_projection: Mat4) -> Self {
        Self {
            size,
            _padding: Padding::default(),
            view_projection,
        }
    }
}
