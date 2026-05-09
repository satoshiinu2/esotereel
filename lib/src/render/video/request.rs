use crate::{
    ClientState, StreamState,
    project::{clipdata::ClipData, timeline::Timeline},
    requests::Request,
};

/// 同一ストリームへの重複リクエストを抑制するための閾値（秒）
const STREAM_FETCH_THRESHOLD: f64 = 0.5;

pub fn request_stream_packets_for_time(
    timeline: &Timeline,
    app_state: &ClientState,
    current_frame: i64,
) -> Vec<Request> {
    let mut requests = Vec::new();

    for layer in &timeline.layers {
        if let Some(clip) = layer.clips.get_at(current_frame) {
            match &clip.clip_data {
                ClipData::Video { path, media_offset } => {
                    let media_seconds = ClipData::get_media_seconds(
                        timeline.fps,
                        clip.position(),
                        current_frame,
                        *media_offset,
                    );

                    if let Some(resource_id_ref) = app_state.path_to_stream.get(path) {
                        // 読み込まれていないならスキップ
                        let StreamState::Loaded(resource_id) = *resource_id_ref else {
                            continue;
                        };

                        // フレームがない場合にのみリクエストを検討
                        if let Some(mut player) = app_state.streams.get_mut(&resource_id) {
                            if player.get_frame_at(media_seconds).is_some() {
                                continue;
                            }

                            // スパム防止：直近で要求した位置とほぼ同じならレスポンス待ちとみなしてスキップ
                            if let Some(last) = player.last_requested_time {
                                if (last - media_seconds).abs() < STREAM_FETCH_THRESHOLD {
                                    continue;
                                }
                            }
                            player.last_requested_time = Some(media_seconds);
                        }

                        // 既にストリームがあるならパケットを要求
                        requests.push(Request::FetchStreamData {
                            resource_id,
                            seek_seconds: media_seconds,
                            count: 10,
                        });
                    } else {
                        // スパム防止でフラグ立てる
                        app_state
                            .path_to_stream
                            .insert(path.to_owned(), StreamState::Loading);

                        // ストリームがないならロードを要求
                        requests.push(Request::LoadStream { path: path.clone() });
                    }
                }
                _ => {}
            }
        }
    }

    requests
}
