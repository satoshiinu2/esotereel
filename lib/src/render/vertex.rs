use bytemuck::{Pod, Zeroable};

/// 頂点データです。
///
/// `bytemuck` の `Pod` / `Zeroable` を実装し、wgpu の頂点バッファに直接書き込めます。
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub tex_coords: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x4,
        2 => Float32x2,
    ];

    /// 頂点バッファレイアウトを返します。
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }

    /// 矩形を構成する 6 頂点（2 三角形）を返します。
    pub fn rect(x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) -> [Vertex; 6] {
        let (x2, y2) = (x + w, y + h);
        [
            Vertex {
                position: [x, y, 0.0],
                color,
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                position: [x2, y, 0.0],
                color,
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [x, y2, 0.0],
                color,
                tex_coords: [0.0, 1.0],
            },
            Vertex {
                position: [x2, y, 0.0],
                color,
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [x2, y2, 0.0],
                color,
                tex_coords: [1.0, 1.0],
            },
            Vertex {
                position: [x, y2, 0.0],
                color,
                tex_coords: [0.0, 1.0],
            },
        ]
    }
}
