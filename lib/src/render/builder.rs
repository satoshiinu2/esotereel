use crate::{project::timeline::Timeline, render::vertex::Vertex};

pub fn build_vertices(timeline: &Timeline, current_frame: i64) -> Vec<Vertex> {
    let mut vertices = vec![];

    for layer in &timeline.layers {
        if let Some(_clip) = layer.get_clip_at_frame(current_frame) {
            // TODO: クリップの状態から作成
            // placeholder
            let color = [1.0, 1.0, 1.0, 1.0];
            let rect = Vertex::rect(
                100.0,
                100.0 + (layer.index as f32 * 60.0),
                200.0,
                50.0,
                color,
            );
            vertices.extend_from_slice(&rect);
        }
    }
    vertices
}
