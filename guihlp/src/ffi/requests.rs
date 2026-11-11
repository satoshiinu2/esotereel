use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use esotereel_lib::{
    decode::streamplayer::FetchState,
    project::{TimelineTick, command::CommandRequest, ids::TimelineId},
    render::video::request::request_stream_packets_for_time,
    requests::Request,
    util::result::{EsotereelError, LockExt},
};

use crate::ffi::{result::FfiResultVoid, state::ClientStateHandle, stringview::FfiStringView};
use crate::network::ClientNetworkHandler;

/// State Mutex は network の `Arc` を取り出す間だけ保持し、送信前に解放する。
/// 送信（ネットワーク内部ロック取得）を State 保持中に行わないため
/// （docs/architecture/lock-order.md）。
fn network_of(ptr_state: *const ClientStateHandle) -> anyhow::Result<Arc<ClientNetworkHandler>> {
    let state = ClientStateHandle::from_ptr(ptr_state);
    let state_guard = state
        .lock()
        .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
    Ok(Arc::clone(&state_guard.network))
} // state_guard はここで drop される

#[unsafe(no_mangle)]
pub extern "C" fn req_test(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let network = network_of(ptr_state)?;

        network.send(&Request::Test);
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn req_new_project(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let network = network_of(ptr_state)?;

        network.send(&Request::NewProject);
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_fetch_frame(
    ptr_state: *const ClientStateHandle,
    timeline_id: TimelineId, // ポインタではなくIDで受け取る
    current_frame: TimelineTick,
    visible_range_start: TimelineTick,
    visible_range_end: TimelineTick,
) -> FfiResultVoid {
    // Keep the C ABI parameter while media requests are driven by the fetch cache.
    let _ = current_frame;

    fn inner(
        ptr_state: *const ClientStateHandle,
        timeline_id: TimelineId,
        visible_range_start: TimelineTick,
        visible_range_end: TimelineTick,
    ) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        // ロックの取り方（docs/architecture/lock-order.md）:
        // State と Project を同時に保持しない。
        // Phase 1: State は Arc の取り出しだけですぐ解放する。
        let state = ClientStateHandle::from_ptr(ptr_state);
        let (network, project_arc, stream_state_map, stream_players, media_fetch_cache) = {
            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            (
                Arc::clone(&state_guard.network),
                Arc::clone(&state_guard.project),
                Arc::clone(&state_guard.stream_state_map),
                Arc::clone(&state_guard.stream_players),
                Arc::clone(&state_guard.media_fetch_cache),
            )
        };

        // Phase 2: Project の read ロックだけでフェッチ要求を組み立てる。
        let req = {
            let project_guard = project_arc.read_or_err()?;
            let project = project_guard
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("project not found"))?;

            project
                .timeline_ref(timeline_id)
                .ok_or_else(|| anyhow::anyhow!("invalid timeline id"))?;

            request_stream_packets_for_time(
                &stream_state_map,
                &stream_players,
                &media_fetch_cache,
            )
        };

        // Phase 3: 重複抑制の記録だけ State を短時間ロックする
        // （この時点では Project ロックは保持していない）。
        let (requests_to_send, send_clips, clips_range) = {
            let state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

            // 組み立てたフェッチ要求のうち、直前と全く同じもの（同一リソースへの
            // 同一レンジ要求）は重複パケットになるため間引く。クリップ範囲要求
            // も、可視範囲が変わっていなければ再送しない。
            let mut requests_to_send = Vec::with_capacity(req.len());
            for r in req.into_iter() {
                match r {
                    Request::FetchStreamData {
                        resource_id,
                        ranges,
                    } => {
                        let is_duplicate = state_guard
                            .last_stream_fetch
                            .get(&resource_id)
                            .map_or(false, |last| *last == ranges);
                        if is_duplicate {
                            log::debug!(
                                "Skip duplicate FetchStreamData for resource_id: {} ranges: {:?}",
                                resource_id,
                                ranges
                            );
                            // 実際には送信しないため、request_stream_packets_for_time が
                            // 設定したフェッチ中状態をアイドルに戻す
                            if let Some(mut player) = stream_players.get_mut(&resource_id)
                            {
                                player.fetch_state = FetchState::Idle;
                            }
                            continue;
                        }
                        state_guard
                            .last_stream_fetch
                            .insert(resource_id, ranges.clone());
                        requests_to_send.push(Request::FetchStreamData {
                            resource_id,
                            ranges,
                        });
                    }
                    other => requests_to_send.push(other),
                }
            }

            let clips_range = visible_range_start..visible_range_end;
            let send_clips = state_guard
                .last_clips_fetch
                .get(&timeline_id)
                .map_or(true, |last| {
                    *last != (visible_range_start, visible_range_end)
                });
            if send_clips {
                state_guard
                    .last_clips_fetch
                    .insert(timeline_id, (visible_range_start, visible_range_end));
            }

            (requests_to_send, send_clips, clips_range)
        };

        // Phase 4: ロックを保持せずに送信する
        for r in &requests_to_send {
            network.send(r);
        }

        if send_clips {
            network.send(&Request::FetchClipsInRange {
                timeline_id,
                range: clips_range,
            });
        } else {
            log::debug!(
                "Skip duplicate FetchClipsInRange for timeline_id: {} range: {:?}",
                timeline_id,
                clips_range
            );
        }

        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| {
        inner(
            ptr_state,
            timeline_id,
            visible_range_start,
            visible_range_end,
        )
    })) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_project_log(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let network = network_of(ptr_state)?;

        network.send(&Request::DebugFetchProjectStruct);
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn req_load_stream(
    ptr_state: *const ClientStateHandle,
    path: FfiStringView,
) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle, path: FfiStringView) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let path = path.as_str()?.to_string();
        let network = network_of(ptr_state)?;

        network.send(&Request::InitStream { path });
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, path))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}
