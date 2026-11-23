use std::{ops::Range, time};

use dashmap::DashMap;
use ordered_float::OrderedFloat;

use crate::{
    StreamState,
    decode::streamplayer::{FetchState, StreamPlayer},
    project::ids::ResourceId,
    render::video::MediaFetchCache,
    requests::Request,
};

/// バッファの十分性判定結果です。
enum BufferNeed {
    /// 十分なバッファがある
    Sufficient,
    /// 不足。この位置から取得が必要
    NeedMore { fetch_from: f64 },
}

/// 必要な時間範囲に基づき、ストリームパケット取得リクエストを構築します。
///
/// プラグインキャッシュから初期化リクエスト（InitStream）と取得対象範囲を収集し、
/// 各リソースごとにバッファ不足分だけを抽出して `Request::FetchStreamData` を生成します。
pub fn request_stream_packets_for_time(
    path_to_stream: &DashMap<String, StreamState>,
    players: &DashMap<ResourceId, StreamPlayer>,
    plugin_cache: &MediaFetchCache,
) -> Vec<Request> {
    use std::collections::HashMap;
    let mut needs_by_resource: HashMap<ResourceId, Vec<Range<f64>>> = HashMap::new();
    let mut init_requests: Vec<Request> = plugin_cache
        .take_init_requests()
        .into_iter()
        .map(|path| Request::InitStream { path })
        .collect();

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

/// 特定リソースの取得リクエストを構築します。
///
/// 必要な窓を先に `player.active_windows` に記録し、デコーダがフェッチ中でない場合のみ
/// 本当に不足している窓だけを抽出してマージし、`Request::FetchStreamData` を返します。
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
        .filter_map(|range| match assess_buffer(&player, &range) {
            BufferNeed::Sufficient => None,
            BufferNeed::NeedMore { fetch_from } => {
                let start = fetch_from.max(range.start);
                (start < range.end).then_some(start..range.end)
            }
        })
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

/// 隣接・重複する範囲を1つに統合します。
fn merge_overlapping_ranges(ranges: &mut Vec<Range<f64>>) {
    ranges.sort_by(|a, b| a.start.total_cmp(&b.start));
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

/// 指定範囲がバッファに十分あるかを評価します。
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

#[cfg(test)]
#[path = "../../../tests/unit/render/video/request_tests.rs"]
mod tests;
