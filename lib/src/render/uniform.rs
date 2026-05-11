use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ScreenUniform {
    pub size: [f32; 2],
    pub _pad: [f32; 2],
}

impl ScreenUniform {
    pub fn new(size: [f32; 2]) -> Self {
        Self { size, _pad: [0.0, 0.0] }
    }
}
