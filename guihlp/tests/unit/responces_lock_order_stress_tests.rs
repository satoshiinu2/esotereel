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
    state::{ClientStateHandle, client_state_project_lock_read, client_state_project_unlock_read},
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
extern "C" fn cb_on_assets_changed() {}

fn make_state() -> Arc<ClientState> {
    let callbacks = crate::GuiCallbacks {
        on_test: cb_on_test,
        mark_dirty_timeline: cb_mark_dirty,
        on_connected: cb_on_connected,
        on_assets_changed: cb_on_assets_changed,
    };
    // ClientState::new はセッションマーカーを data_dir へ書くため、実データ
    // ディレクトリではなくプロセス専用の一時ワークディレクトリを使う。
    let working_dir = std::env::temp_dir().join(format!(
        "esotereel-guihlp-lock-order-test-{}",
        std::process::id()
    ));
    Arc::new(ClientState::new(
        callbacks,
        Directories::new(None, Some(working_dir)),
    ))
}

/// 本番のネットワーク受信と同じ経路（rkyv シリアライズ -> archived ->
/// on_responce_recveve）でレスポンスを1件処理する。
fn handle_response(state: &Arc<ClientState>, resp: &Response) {
    let mut serializer = AllocSerializer::<8192>::default();
    serializer
        .serialize_value(resp)
        .expect("serialize response");
    let bytes = serializer.into_serializer().into_inner();
    let archived = rkyv::check_archived_root::<Response>(&bytes).expect("archive check");
    on_responce_recveve(archived, state).expect("response handling must succeed");
}

fn seed_project(state: &Arc<ClientState>) -> (TimelineId, LayerId) {
    let mut project = Project::new();
    let timeline_id = project.insert_timeline(60.0);
    let metas = project.timelines_meta();
    let layer_id = metas[0].layers[0].id;
    handle_response(state, &Response::ProjectMeta { timelines: metas });
    (timeline_id, layer_id)
}

#[test]
fn concurrent_response_handling_and_gui_pattern_do_not_deadlock() {
    let state = make_state();
    let (timeline_id, layer_id) = seed_project(&state);

    // C++ 側と同じ形の生ハンドル（Arc<ClientState> の into_raw）
    let raw_state = Arc::into_raw(Arc::clone(&state)) as *const ClientStateHandle;
    let raw_addr = raw_state as usize;

    let (done_tx, done_rx) = std::sync::mpsc::channel::<&'static str>();

    // ワーカーA: ネットワークスレッド役。
    // プロジェクトを変更するレスポンスを繰り返し適用する。
    // ProjectMeta は state.project のインプレース更新も発生させる。
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
            let value = unsafe { settings_get_value(raw, FfiStringView::from_str(key)) };
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
            let result = unsafe { req_fetch_frame(raw, timeline_id, 0, 0, 1000) };
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
        let _ = Arc::from_raw(raw_state as *const ClientState);
    }
}
