use crate::{
    ClientState,
    project::{clipdata::ClipData, timeline::Timeline},
    render::vertex::Vertex,
};

pub struct VertexBatch {
    pub vertices: Vec<Vertex>,
    pub texture_id: u32, // ここでどのテクスチャを使用するか識別する
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
            let rect = Vertex::rect(100.0, 100.0, 400.0, 300.0, color);
            batches.push(VertexBatch {
                vertices: rect.to_vec(),
                texture_id,
            });
        }
    }
    batches
}
