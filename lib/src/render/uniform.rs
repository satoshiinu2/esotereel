use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use crate::util::Padding;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ScreenUniform {
    pub size: [f32; 2],
    _padding: Padding<8>,
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
