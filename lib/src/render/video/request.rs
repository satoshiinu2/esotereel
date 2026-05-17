use std::{ops::Range, time};

use crate::{
    ClientState, StreamState,
    decode::streamplayer::{FetchState, StreamPlayer},
    project::{clip::Clip, clip_data::ClipData, timeline::Timeline},
    requests::Request,
};

enum BufferNeed {
    Sufficient,                   // 十分
    NeedMore { fetch_from: f64 }, // 不足、この位置から取得が必要
    Stale,                        // 古い（シーク後などバッファが現在位置より前）
}

pub const BUFFER_LOOKAHEAD_THRESHOLD: f64 = 1.0; // バッファの先読み量

pub fn request_stream_packets_for_time(
    timeline: &Timeline,
    app_state: &ClientState,
    frame_range: Range<i64>,
) -> Vec<Request> {
    timeline
        .layers
        .iter()
        .filter_map(|layer| {
            layer
                .clips
                .get_clips_in_range(frame_range.clone())
                .first()
                .cloned()
        })
        .filter_map(|clip| {
            collect_request_for_clip(timeline, app_state, &clip, frame_range.clone())
        })
        .collect()
}

fn collect_request_for_clip(
    timeline: &Timeline,
    app_state: &ClientState,
    clip: &Clip,
    frame_range: Range<i64>,
) -> Option<Request> {
    let (path, media_offset) = match &clip.clip_data {
        ClipData::Video { path, media_offset } => Some((path, media_offset)),
        _ => None,
    }?;

    if let Some(resource_id_ref) = app_state.path_to_stream.get(path) {
        let StreamState::Loaded(resource_id) = *resource_id_ref else {
            return None;
        };

        let start_seconds = ClipData::get_media_seconds(
            timeline.fps,
            clip.position(),
            frame_range.start,
            *media_offset,
        );

        if let Some(mut player) = app_state.streams.get_mut(&resource_id) {
            if player.fetch_state.is_active() {
                return None;
            }

            let buffer_need = assess_buffer(&player, start_seconds);

            if player.fetch_state.is_active() {
                return None;
            }

            let fetch_from = match buffer_need {
                BufferNeed::Sufficient => return None,
                BufferNeed::Stale => start_seconds, // シーク後は現在位置から取り直し
                BufferNeed::NeedMore { fetch_from } => {
                    if player.fetch_state.is_active() {
                        return None;
                    }
                    fetch_from
                }
            };

            let seek_range_sec = fetch_from..fetch_from + BUFFER_LOOKAHEAD_THRESHOLD;

            player.fetch_state = FetchState::Fetching {
                requested_at: time::Instant::now(),
                seek_range_sec: seek_range_sec.clone(),
            };

            Some(Request::FetchStreamData {
                resource_id,
                seek_range_sec: seek_range_sec.clone(),
            })
        } else {
            None
        }
    } else {
        app_state
            .path_to_stream
            .insert(path.to_owned(), StreamState::Loading);
        Some(Request::InitStream {
            path: path.to_owned(),
        })
    }
}

fn assess_buffer(player: &StreamPlayer, current_seconds: f64) -> BufferNeed {
    let buffer_front = player.frames.first_key_value().map(|(t, _)| t.0);
    let buffer_end = player.frames.last_key_value().map(|(t, _)| t.0);

    match (buffer_front, buffer_end) {
        (None, _) | (_, None) => BufferNeed::NeedMore {
            fetch_from: current_seconds,
        },
        (Some(front), Some(end)) => {
            if current_seconds < front || current_seconds > end {
                return BufferNeed::Stale;
            }
            if player.get_frame_at(current_seconds).is_none() {
                return BufferNeed::Stale; // NeedMoreではなくStale
            }
            let buffered = end - current_seconds;
            if buffered < BUFFER_LOOKAHEAD_THRESHOLD {
                BufferNeed::NeedMore { fetch_from: end }
            } else {
                BufferNeed::Sufficient
            }
        }
    }
}
