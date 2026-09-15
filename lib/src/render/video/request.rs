use std::{ops::Range, time};

use dashmap::DashMap;
use ordered_float::OrderedFloat;

use crate::{
    StreamState,
    decode::streamplayer::{FetchState, StreamPlayer},
    project::{Timeline, TimelineTick, ids::ResourceId},
    render::video::MediaFetchCache,
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
    path_to_stream: &DashMap<String, StreamState>,
    players: &DashMap<ResourceId, StreamPlayer>,
    frame_range: Range<TimelineTick>,
    plugin_cache: &MediaFetchCache,
) -> Vec<Request> {
    use std::collections::HashMap;
    let mut needs_by_resource: HashMap<ResourceId, Vec<Range<f64>>> = HashMap::new();
    let mut init_requests = Vec::new();

    for (path, range) in plugin_cache.take_all() {
        match path_to_stream.get(&path).map(|r| *r) {
            Some(StreamState::Loaded(resource_id)) => {
                needs_by_resource
                    .entry(resource_id)
                    .or_default()
                    .push(range);
            }
            None => {
                path_to_stream.insert(path.clone(), StreamState::Loading);
                init_requests.push(Request::InitStream { path });
            }
            _ => {}
        }
    }

    for mut entry in players.iter_mut() {
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
                collect_request_for_resource(players, resource_id, needed)
            }),
    );
    requests
}

fn collect_request_for_resource(
    players: &DashMap<ResourceId, StreamPlayer>,
    resource_id: ResourceId,
    needed_ranges: Vec<Range<f64>>,
) -> Option<Request> {
    let mut player = players.get_mut(&resource_id)?;

    // このフレームで必要な窓は、フェッチ中でも消されないよう先に記録
    player.active_windows = needed_ranges.clone();

    if player.fetch_state.is_active() {
        return None; // デコーダは1本なので進行中バッチが終わるまで待つ
    }

    // 本当に足りていない窓だけを抜き出してまとめて送る
    let mut ranges: Vec<Range<f64>> = needed_ranges
        .into_iter()
        .filter(|r| !matches!(assess_buffer(&player, r), BufferNeed::Sufficient))
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

fn assess_buffer(player: &StreamPlayer, range: &Range<f64>) -> BufferNeed {
    if player.get_frame_at(range.start).is_none() {
        return BufferNeed::NeedMore {
            fetch_from: range.start,
        };
    }

    let mut end = range.start;
    for (&OrderedFloat(t), _) in player.frames.range(OrderedFloat(range.start)..) {
        if t - end > 0.2 {
            break;
        }
        end = t;
    }

    if end < range.end {
        BufferNeed::NeedMore { fetch_from: end }
    } else {
        BufferNeed::Sufficient
    }
}
