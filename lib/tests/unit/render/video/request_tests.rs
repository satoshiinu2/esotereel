use super::*;

#[test]
fn texture_inspection_does_not_consume_requests_and_init_is_sent_once() {
    let cache = MediaFetchCache::default();
    cache.record(1, "test.mp4".into(), 2.0..3.0);
    cache.record_init_request("test.mp4".into());
    assert_eq!(cache.snapshot().len(), 1);
    let states = DashMap::new();
    states.insert("test.mp4".into(), StreamState::Loading);
    let players = DashMap::new();
    let requests = request_stream_packets_for_time(&states, &players, &cache);
    assert_eq!(requests.len(), 1);
    assert!(matches!(&requests[0], Request::InitStream { path } if path == "test.mp4"));
    assert!(cache.take_all().is_empty());
    assert!(request_stream_packets_for_time(&states, &players, &cache).is_empty());
}

#[test]
fn requests_only_the_unbuffered_tail_of_a_needed_range() {
    let players = DashMap::new();
    let mut player =
        StreamPlayer::new_from_metadata(ffmpeg_next::codec::Id::H264, 32, 24, 1.0 / 30.0, &[])
            .unwrap();
    let frame =
        ffmpeg_next::util::frame::video::Video::new(ffmpeg_next::format::Pixel::RGBA, 32, 24);
    player.frames.insert(OrderedFloat(2.0), frame.clone());
    player.frames.insert(OrderedFloat(2.1), frame);
    players.insert(7, player);

    let states = DashMap::new();
    states.insert("test.mp4".into(), StreamState::Loaded(7));
    let cache = MediaFetchCache::default();
    cache.record(1, "test.mp4".into(), 2.0..3.0);

    let requests = request_stream_packets_for_time(&states, &players, &cache);
    assert_eq!(requests.len(), 1);
    assert!(matches!(
        &requests[0],
        Request::FetchStreamData { resource_id: 7, ranges }
            if ranges == &vec![2.1..3.0]
    ));
}
