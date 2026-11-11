use std::{
    ops::Range,
    path::Path,
    sync::{Arc, Mutex, RwLock},
};

use colored::Colorize;
use esotereel_lib::{
    StreamState,
    decode::streamplayer::{FetchState, StreamPlayer},
    plugin::{NamespacedID, property::value::FieldValue},
    project::{
        Project,
        clip::{Clip, ClipBindingValue},
        ids::{ClipId, LayerFolderId, LayerId, TimelineId},
        layer::LayerMeta,
        layer_outline::{Meta, OutlineNode},
        timeline::TimelineMeta,
    },
    responces::ArchivedResponse,
    util::result::{EsotereelError, EsotereelResult, LockExt},
};
use log::info;
use rkyv::Deserialize;

use crate::state::ClientState;

/// State Mutex は project の `Arc` とコールバック関数ポインタを取り出す間だけ
/// 保持し、Project ロックを取得する前に解放する。
/// ネットワークスレッドが State を保持したまま Project を待つと、Project の
/// read ガードを保持して State を取りに来る GUI スレッドと AB-BA デッドロック
/// になるため（docs/architecture/lock-order.md）。
fn project_and_callback(
    state: &Mutex<ClientState>,
) -> EsotereelResult<(Arc<RwLock<Option<Project>>>, extern "C" fn(TimelineId))> {
    let state_guard = state
        .lock()
        .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
    Ok((
        Arc::clone(&state_guard.project),
        state_guard.gui_callbacks.mark_dirty_timeline,
    ))
} // state_guard はここで drop される

/// デコード等の重い処理を State ロックなしで行うため、stream_players の
/// `Arc` だけを短時間の State ロックで取り出す。
fn stream_players_of(
    state: &Mutex<ClientState>,
) -> EsotereelResult<Arc<dashmap::DashMap<esotereel_lib::project::ids::ResourceId, StreamPlayer>>> {
    let state_guard = state
        .lock()
        .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
    Ok(Arc::clone(&state_guard.stream_players))
}

pub(super) fn on_responce_recveve(
    responce: &ArchivedResponse,
    state: &Mutex<ClientState>,
) -> EsotereelResult<()> {
    match responce {
        ArchivedResponse::Test => {}
        ArchivedResponse::ProjectMeta { timelines } => {
            let timeline_metas: Vec<TimelineMeta> = timelines
                .iter()
                .map(|archived| {
                    let layer_metas: Vec<LayerMeta> = archived
                        .layers
                        .iter()
                        .map(|al| LayerMeta {
                            id: al.id,
                            name: al.name.as_str().to_string(),
                            enabled: al.enabled,
                        })
                        .collect();

                    let outline = archived.outline.deserialize(&mut rkyv::Infallible).unwrap();

                    TimelineMeta {
                        id: archived.id,
                        fps: archived.fps,
                        layers: layer_metas,
                        outline,
                    }
                })
                .collect();
            info!("timeline_metas: {:?}", &timeline_metas);

            let real_project = Project::from_meta(timeline_metas);
            let timeline_len = real_project.timeline_count();
            let project_arc = Arc::new(RwLock::new(Some(real_project)));

            let mut state_guard = state
                .lock()
                .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
            state_guard.project = project_arc;
            // プロジェクトが再初期化されたため、タイムラインごとの最後の
            // FetchClipsInRange 送信記録をクリアする（新しいプロジェクトでの
            // 重複送信抑制が誤って働かないようにする）
            state_guard.last_clips_fetch.clear();

            let fn_ptr = state_guard.gui_callbacks.mark_dirty_timeline;
            drop(state_guard);

            for i in 0..timeline_len {
                fn_ptr(i as TimelineId);
            }
        }
        ArchivedResponse::UpdateClip { timeline_id, clips } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                let mut sources = Vec::new();
                for (layer_id, archived_clip) in clips.iter() {
                    let clip = archived_clip.deserialize(&mut rkyv::Infallible).unwrap();
                    if let Some(source) = video_source_path(&clip) {
                        sources.push(source);
                    }
                    timeline.upsert_clip_from_network(*layer_id, clip);
                }
                // クライアント側のアセットレジストリにも動画ソースを登録し、
                // Project.assets をサーバーと整合したカタログとして保つ。
                for source in sources {
                    let _ = project.register_asset(Path::new(&source));
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }
        ArchivedResponse::UpdateClipProperty {
            timeline_id,
            clip_id,
            key,
            value,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let mut new_source = None;
            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                let clip_id: ClipId = clip_id.deserialize(&mut rkyv::Infallible).unwrap();
                let key: NamespacedID = key.deserialize(&mut rkyv::Infallible).unwrap();
                let value: ClipBindingValue = value.deserialize(&mut rkyv::Infallible).unwrap();

                // プロパティ変更が動画ソースなら、タイムライン貸し出しを解放した後に
                // クライアントのアセットレジストリにも反映する。
                new_source = video_source_from_property(&key, &value);

                if let Some(clip) = timeline.get_clip_mut(clip_id) {
                    clip.set_property_value(key, value)?;
                }
            }

            if let Some(source) = new_source {
                let _ = project.register_asset(Path::new(&source));
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }
        ArchivedResponse::RemoveClip {
            timeline_id,
            clip_ids,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                for (layer_id, clip_id) in clip_ids.iter() {
                    timeline.remove_clip_by_id_in(*layer_id, *clip_id);
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }
        ArchivedResponse::CommandRejected {
            timeline_id,
            clip_id,
            current_position,
            current_duration,
            current_layer_id: _,
            ..
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            let timeline = project
                .timeline_mut(*timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

            // サーバーが通知した正しい状態でローカルのクリップを上書きし、
            // 次の操作でステールネスエラーが再発しないようにする
            if let Some(clip) = timeline.get_clip_mut(*clip_id) {
                clip.set_position(*current_position);
                clip.duration = *current_duration;
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }

        ArchivedResponse::UpdateLayer {
            timeline_id,
            layers,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                for archived_layer in layers.iter() {
                    let meta = LayerMeta {
                        id: archived_layer.id,
                        name: archived_layer.name.as_str().to_string(),
                        enabled: archived_layer.enabled,
                    };
                    timeline.apply_layer_meta(meta);
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }
        ArchivedResponse::RemoveLayer {
            timeline_id,
            layer_ids,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                for archived_id in layer_ids.iter() {
                    let id: LayerId = archived_id.deserialize(&mut rkyv::Infallible).unwrap();
                    timeline.remove_layer_local(id); // サーバー側のremove_layerと違い、
                    // parentのchildren書き換えは不要
                    // (親も別途upsertで送られてくるので)
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }

        ArchivedResponse::UpdateOutline {
            timeline_id,
            folders,
            children,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;

                for (id, meta) in folders.iter() {
                    let meta: Meta = meta.deserialize(&mut rkyv::Infallible).unwrap();
                    timeline.apply_outline_folder_meta(*id, meta);
                }
                for (parent, kids) in children.iter() {
                    let parent: Option<LayerFolderId> =
                        parent.deserialize(&mut rkyv::Infallible).unwrap();
                    let kids: Vec<OutlineNode> = kids.deserialize(&mut rkyv::Infallible).unwrap();
                    timeline.apply_outline_children(parent, kids);
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }

        ArchivedResponse::Removes {
            timeline_id,
            folder_ids,
        } => {
            let (project_arc, fn_ptr) = project_and_callback(state)?;
            let mut project_guard = project_arc.write_or_err()?;
            let project = project_guard
                .as_mut()
                .ok_or(EsotereelError::ProjectNotFound)?;

            {
                let timeline = project
                    .timeline_mut(*timeline_id)
                    .ok_or(EsotereelError::TimelineNotFound(*timeline_id))?;
                for archived_id in folder_ids.iter() {
                    let id: LayerFolderId = archived_id.deserialize(&mut rkyv::Infallible).unwrap();
                    timeline.remove_outline_folder_local(id);
                }
            }

            drop(project_guard);
            (fn_ptr)(*timeline_id);
        }

        ArchivedResponse::StreamMetadata {
            path,
            resource_id,
            codec_id,
            width,
            height,
            time_base,
            extradata,
            ..
        } => {
            let codec_id = unsafe { std::mem::transmute(*codec_id) };

            let player = StreamPlayer::new_from_metadata(
                codec_id,
                *width,
                *height,
                *time_base,
                extradata.as_slice(),
            )
            .map_err(|e| esotereel_lib::util::result::EsotereelError::IoError(e.to_string()))?;

            {
                // この State ロックは DashMap への挿入のみで、他のロックを
                // 取得しないため短時間保持のままにできる
                let state = state
                    .lock()
                    .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
                state.stream_players.insert(*resource_id, player);
                state
                    .stream_state_map
                    .insert(path.as_ref().to_owned(), StreamState::Loaded(*resource_id));
                // 新しいストリームプレイヤーが割り当てられたため、過去の
                // FetchStreamData 送信記録をクリアする（別メディアの同じ
                // resource_id に旧記録が誤って効かないようにする）
                state.last_stream_fetch.remove(resource_id);
            }

            log::info!(
                "Player initialized for resource: {} ({})",
                resource_id,
                path
            );
        }
        ArchivedResponse::StreamData {
            resource_id,
            data,
            pts,
            dts,
            is_key,
            discontinuous,
            generation,
        } => {
            let pts: Option<i64> = pts.deserialize(&mut rkyv::Infallible).unwrap();
            let dts: Option<i64> = dts.deserialize(&mut rkyv::Infallible).unwrap();

            {
                // デコードは重い処理のため、State ロックを保持したまま行わない
                let stream_players = stream_players_of(state)?;
                if let Some(mut player) = stream_players.get_mut(resource_id) {
                    // パケットをデコードしてフレームを取得
                    player
                        .process_packet(data, pts, dts, *is_key, *discontinuous, *generation)
                        .map_err(|e| EsotereelError::DecodeError(e.to_string()))?;
                }
            }
        }
        ArchivedResponse::StreamDataEnd {
            resource_id,
            fetched_ranges,
            generation: _,
        } => {
            let fetched_ranges: Vec<Range<f64>> =
                fetched_ranges.deserialize(&mut rkyv::Infallible).unwrap();

            let stream_players = stream_players_of(state)?;
            if let Some(mut player) = stream_players.get_mut(resource_id) {
                player.fetch_state = FetchState::Idle;
                player.free_no_needed_frames(&fetched_ranges); // Vec対応版
            }
        }
        ArchivedResponse::DebugProjectStruct(server_str) => {
            let (project_arc, _) = project_and_callback(state)?;
            let project_guard = project_arc.read_or_err()?;

            if let Some(project) = project_guard.as_ref() {
                let client_str = format!("{:#?}", project);

                info!("Client: {}", client_str.green());
            } else {
                info!("Client project is None");
            }

            if let Some(server_str) = server_str.as_deref() {
                info!("Server: {}", server_str.red());
            } else {
                info!("Server project is None");
            }
        }
    }
    Ok(())
}

/// クリッププロパティから動画ソース（`std:video:media/source`）のパス文字列を取り出す。
fn video_source_path(clip: &Clip) -> Option<String> {
    let media_key = NamespacedID::new("std", "video:media").unwrap();
    let FieldValue::Map(media_map) = clip.properties.get(&media_key)?.in_frame_of()? else {
        return None;
    };
    let FieldValue::String(source) = media_map.get("source")? else {
        return None;
    };
    if source.is_empty() {
        return None;
    }
    Some(source.clone())
}

/// プロパティ変更（ルートキー + 値）から動画ソースパスを取り出す。
/// `key` が動画メディアグループのルートで、`value` がそのマップ値である場合にのみ
/// `source` メンバーを返す。
fn video_source_from_property(key: &NamespacedID, value: &ClipBindingValue) -> Option<String> {
    let media_key = NamespacedID::new("std", "video:media").unwrap();
    if key != &media_key {
        return None;
    }
    let FieldValue::Map(media_map) = value.in_frame_of()? else {
        return None;
    };
    let FieldValue::String(source) = media_map.get("source")? else {
        return None;
    };
    if source.is_empty() {
        return None;
    }
    Some(source.clone())
}

#[cfg(test)]
mod lock_order_stress_tests {
    //! ロック順序の回帰テスト（docs/architecture/lock-order.md）。
    //!
    //! 旧実装では、ネットワークスレッドのレスポンス処理が
    //! State -> Project の順にロックを取り、GUI スレッドが Project の
    //! read ガードを保持したまま State を取る FFI（設定取得等）を呼ぶと
    //! AB-BA デッドロックした。本テストはそのパターンを API レベルで
    //! 並行に再現し、制限時間内に全ワーカーが完了することを確認する。
    //! タイムアウト = デッドロック検出として失敗扱いにする。

    use super::*;
    use crate::ffi::{
        requests::req_fetch_frame,
        settings::settings_get_value,
        state::{
            ClientStateHandle, client_state_project_lock_read,
            client_state_project_unlock_read,
        },
        stringview::FfiStringView,
    };
    use esotereel_lib::{dirs::Directories, responces::Response};
    use rkyv::ser::{Serializer, serializers::AllocSerializer};
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    static DIRTY_CALLS: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn cb_on_test() {}
    extern "C" fn cb_mark_dirty(_id: TimelineId) {
        DIRTY_CALLS.fetch_add(1, Ordering::Relaxed);
    }
    extern "C" fn cb_on_connected() {}

    fn make_state() -> Arc<Mutex<ClientState>> {
        let callbacks = crate::GuiCallbacks {
            on_test: cb_on_test,
            mark_dirty_timeline: cb_mark_dirty,
            on_connected: cb_on_connected,
        };
        Arc::new(Mutex::new(ClientState::new(
            callbacks,
            Directories::new(None, None),
        )))
    }

    /// 本番のネットワーク受信と同じ経路（rkyv シリアライズ -> archived ->
    /// on_responce_recveve）でレスポンスを1件処理する。
    fn handle_response(state: &Arc<Mutex<ClientState>>, resp: &Response) {
        let mut serializer = AllocSerializer::<8192>::default();
        serializer.serialize_value(resp).expect("serialize response");
        let bytes = serializer.into_serializer().into_inner();
        let archived = rkyv::check_archived_root::<Response>(&bytes).expect("archive check");
        on_responce_recveve(archived, state).expect("response handling must succeed");
    }

    fn seed_project(state: &Arc<Mutex<ClientState>>) -> (TimelineId, LayerId) {
        let mut project = Project::new();
        let timeline_id = project.insert_timeline(60.0);
        let metas = project.timelines_meta();
        let layer_id = metas[0].layers[0].id;
        handle_response(
            state,
            &Response::ProjectMeta {
                timelines: metas,
            },
        );
        (timeline_id, layer_id)
    }

    #[test]
    fn concurrent_response_handling_and_gui_pattern_do_not_deadlock() {
        let state = make_state();
        let (timeline_id, layer_id) = seed_project(&state);

        // C++ 側と同じ形の生ハンドル（Arc<Mutex<ClientState>> の into_raw）
        let raw_state = Arc::into_raw(Arc::clone(&state)) as *const ClientStateHandle;
        let raw_addr = raw_state as usize;

        let (done_tx, done_rx) = std::sync::mpsc::channel::<&'static str>();

        // ワーカーA: ネットワークスレッド役。
        // プロジェクトを変更するレスポンスを繰り返し適用する。
        // ProjectMeta は state.project の差し替えも発生させる。
        let a_state = Arc::clone(&state);
        let a_tx = done_tx.clone();
        std::thread::spawn(move || {
            for i in 0..150 {
                handle_response(
                    &a_state,
                    &Response::UpdateLayer {
                        timeline_id,
                        layers: vec![LayerMeta {
                            id: layer_id,
                            name: format!("layer-{i}"),
                            enabled: true,
                        }],
                    },
                );
                if i % 25 == 24 {
                    seed_project(&a_state);
                }
            }
            let _ = a_tx.send("responses");
        });

        // ワーカーB: GUI スレッド役。
        // Project の read ガードを保持したまま、State ロックを取る FFI
        // （設定取得）を呼ぶ。旧順序ではここでデッドロックした。
        let b_tx = done_tx.clone();
        std::thread::spawn(move || {
            let raw = raw_addr as *const ClientStateHandle;
            for _ in 0..150 {
                let result = unsafe { client_state_project_lock_read(raw) };
                assert!(result.is_ok, "project lock_read must succeed");
                let guard_ptr = unsafe { result.value.ok };
                assert!(!guard_ptr.is_null());

                // ガード保持中に State ロックを取る FFI を呼ぶ
                let key = "core:log.level";
                let value = unsafe {
                    settings_get_value(raw, FfiStringView::from_str(key))
                };
                assert!(value.is_ok, "settings_get_value must succeed");

                let unlock = unsafe { client_state_project_unlock_read(guard_ptr) };
                assert!(unlock.is_ok, "project unlock_read must succeed");
            }
            let _ = b_tx.send("gui-pattern");
        });

        // ワーカーC: req_fetch_frame 役（State + Project を取り、送信は
        // ロック解放後に行う経路）を繰り返す。
        let c_tx = done_tx.clone();
        std::thread::spawn(move || {
            let raw = raw_addr as *const ClientStateHandle;
            for _ in 0..100 {
                let result =
                    unsafe { req_fetch_frame(raw, timeline_id, 0, 0, 1000) };
                assert!(result.is_ok, "req_fetch_frame must succeed");
            }
            let _ = c_tx.send("fetch-frame");
        });

        drop(done_tx);

        // 全ワーカーの完了を制限時間内に待つ。タイムアウトはデッドロック。
        let mut finished: Vec<&'static str> = Vec::new();
        for _ in 0..3 {
            match done_rx.recv_timeout(Duration::from_secs(60)) {
                Ok(name) => finished.push(name),
                Err(e) => panic!(
                    "deadlock suspected: workers did not finish in time ({e}); finished: {finished:?}"
                ),
            }
        }
        assert_eq!(finished.len(), 3);
        assert!(DIRTY_CALLS.load(Ordering::Relaxed) > 0);

        // 生ハンドルの Arc を回収する
        unsafe {
            let _ = Arc::from_raw(raw_state as *const Mutex<ClientState>);
        }
    }
}
