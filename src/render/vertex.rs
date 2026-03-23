use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x4,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }

    pub fn rect(x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) -> [Vertex; 6] {
        let (x2, y2) = (x + w, y + h);
        [
            Vertex {
                position: [x, y],
                color,
            },
            Vertex {
                position: [x2, y],
                color,
            },
            Vertex {
                position: [x, y2],
                color,
            },
            Vertex {
                position: [x2, y],
                color,
            },
            Vertex {
                position: [x2, y2],
                color,
            },
            Vertex {
                position: [x, y2],
                color,
            },
        ]
    }
}
