use crate::project::timeline::Timeline;

pub fn request_stream_packets_for_time(timeline: &Timeline, current_time: i64) {
    for layer in &timeline.layers {
        if let Some(clip) = layer.get_clip_at_frame(current_time) {
            let offset_in_clip = current_time - clip.position;

            // サーバーに「このクリップのこの時間からパケットをくれ」と頼む
            // 実際には resource_id と、計算した秒数を送る
            let seek_seconds = (offset_in_clip as f64) / 1000.0; // ミリ秒想定

            // 1. SeekStream { resource_id, seconds: seek_seconds }
            // 2. FetchStreamData { resource_id, count: 30 }
        }
    }
}
