use crate::{
    ClientState,
    project::{clip_data::ClipData, timeline::Timeline},
    render::vertex::Vertex,
};
use glam::{EulerRot, Mat4, Quat, Vec3};

pub struct VertexBatch {
    pub vertices: Vec<Vertex>,
    pub texture_id: u32, // ここでどのテクスチャを使用するか識別する
    pub transform: Mat4,
}

pub fn build_vertices(
    timeline: &Timeline,
    app_state: &ClientState,
    current_frame: i64,
) -> Vec<VertexBatch> {
    let mut batches = vec![];

    for layer in timeline.layers.get_sorted_iter() {
        if let Some(clip) = layer.clips.get_at(current_frame) {
            let texture_id = if let ClipData::Video { path, .. } = &clip.clip_data {
                app_state
                    .path_to_stream
                    .get(path)
                    .and_then(|s| s.as_option())
                    .unwrap_or(u32::MAX)
            } else {
                u32::MAX
            };

            let color = [1.0, 1.0, 1.0, 1.0];
            
            let rect = Vertex::rect(0.0, 0.0, 1.0, 1.0, color);

            let clip_trans = clip.translates.get_translate_at();

            let transform = if let Some(t) = clip_trans {
                Mat4::from_scale_rotation_translation(
                    Vec3::from_array(t.scale),
                    Quat::from_euler(EulerRot::XYZ, t.rotation[0], t.rotation[1], t.rotation[2]),
                    Vec3::from_array(t.position),
                )
            } else {
                Mat4::IDENTITY
            };

            batches.push(VertexBatch {
                vertices: rect.to_vec(),
                texture_id,
                transform,
            });
        }
    }
    batches
}
