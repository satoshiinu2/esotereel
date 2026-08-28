use std::{ops::Range, time};

use ordered_float::OrderedFloat;

use crate::{
    ClientState, StreamState,
    decode::streamplayer::{FetchState, StreamPlayer},
    project::{Timeline, TimelineTick, clip::ClipData, ids::ResourceId},
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
    frame_range: Range<TimelineTick>,
) -> Vec<Request> {
    use std::collections::HashMap;
    let mut needs_by_resource: HashMap<ResourceId, Vec<f64>> = HashMap::new();
    let mut init_requests = Vec::new();

    // 必要なパケットをresource_idごとにまとめる
    for (&layer_id, _) in timeline.iter_layers() {
        let Some(clip) = timeline.get_clip_at(layer_id, frame_range.start) else {
            continue;
        };
        let ClipData::Video { path, media_offset } = &clip.data else {
            continue;
        };

        match app_state.path_to_stream.get(path).map(|r| *r) {
            Some(StreamState::Loaded(resource_id)) => {
                let start_seconds = ClipData::get_media_seconds(
                    timeline.tps,
                    clip.position,
                    frame_range.start,
                    *media_offset,
                );
                needs_by_resource
                    .entry(resource_id)
                    .or_default()
                    .push(start_seconds);
            }
            None => {
                app_state
                    .path_to_stream
                    .insert(path.to_owned(), StreamState::Loading);
                init_requests.push(Request::InitStream {
                    path: path.to_owned(),
                });
            }
            _ => {}
        }
    }

    // 今フレーム参照されなかったストリームの active_windows をクリア
    for mut entry in app_state.streams.iter_mut() {
        let (resource_id, player) = entry.pair_mut();
        if !needs_by_resource.contains_key(resource_id) {
            player.active_windows.clear();
        }
    }

    let mut requests = init_requests;
    requests.extend(
        needs_by_resource
            .into_iter()
            .filter_map(|(resource_id, needed)| {
                collect_request_for_resource(app_state, resource_id, needed)
            }),
    );
    requests
}

fn collect_request_for_resource(
    app_state: &ClientState,
    resource_id: u32,
    needed_seconds: Vec<f64>,
) -> Option<Request> {
    let mut player = app_state.streams.get_mut(&resource_id)?;

    // このフレームで必要な窓は、フェッチ中でも消されないよう先に記録
    player.active_windows = needed_seconds
        .iter()
        .map(|&s| s..s + BUFFER_LOOKAHEAD_THRESHOLD)
        .collect();

    if player.fetch_state.is_active() {
        return None; // デコーダは1本なので進行中バッチが終わるまで待つ
    }

    // 本当に足りていない窓だけを抜き出してまとめて送る
    let mut ranges: Vec<Range<f64>> = needed_seconds
        .into_iter()
        .filter(|&s| !matches!(assess_buffer(&player, s), BufferNeed::Sufficient))
        .map(|s| s..s + BUFFER_LOOKAHEAD_THRESHOLD)
        .collect();

    if ranges.is_empty() {
        return None;
    }

    merge_overlapping_ranges(&mut ranges); // 隣接/重複区間は1つに統合しておく

    player.fetch_state = FetchState::Fetching {
        requested_at: time::Instant::now(),
        seek_ranges: ranges.clone(),
    };

    Some(Request::FetchStreamData {
        resource_id,
        ranges,
    })
}

fn merge_overlapping_ranges(ranges: &mut Vec<Range<f64>>) {
    ranges.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
    let mut merged: Vec<Range<f64>> = Vec::new();
    for r in ranges.drain(..) {
        if let Some(last) = merged.last_mut() {
            if r.start <= last.end + 0.001 {
                last.end = last.end.max(r.end);
                continue;
            }
        }
        merged.push(r);
    }
    *ranges = merged;
}

fn assess_buffer(player: &StreamPlayer, current_seconds: f64) -> BufferNeed {
    // current_seconds 時点(または直近)のフレームが存在するか確認
    if player.get_frame_at(current_seconds).is_none() {
        return BufferNeed::NeedMore {
            fetch_from: current_seconds,
        };
    }

    // current_seconds 位置から連続して存在するバッファの末尾を探す
    let mut end = current_seconds;
    for (&OrderedFloat(t), _) in player.frames.range(OrderedFloat(current_seconds)..) {
        // フレーム間の間隔が大きく開いたらバッファの切れ目と判断 (例: 0.2秒以上)
        if t - end > 0.2 {
            break;
        }
        end = t;
    }

    let buffered = end - current_seconds;
    if buffered < BUFFER_LOOKAHEAD_THRESHOLD {
        BufferNeed::NeedMore { fetch_from: end }
    } else {
        BufferNeed::Sufficient
    }
}
