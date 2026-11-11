use esotereel_lib::{
    dirs::Directories,
    project::Project,
    state::HostState,
    util::result::{EsotereelError, LockExt},
};

use crate::{
    GuiCallbacks,
    ffi::{
        log_if_panicked,
        result::{FfiResult, FfiResultVoid},
        stringview::FfiStringView,
    },
    state::ClientState,
};
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{Arc, Mutex, RwLock, RwLockReadGuard},
};

pub struct ClientStateHandle(pub Arc<Mutex<ClientState>>);
pub type OptionProject = Option<Project>;

impl ClientStateHandle {
    pub fn from_ptr(ptr_state: *const ClientStateHandle) -> Arc<Mutex<ClientState>> {
        let raw = ptr_state as *const Mutex<ClientState>;
        unsafe {
            Arc::increment_strong_count(raw);
            Arc::from_raw(raw)
        }
    }
}

impl std::ops::Deref for ClientStateHandle {
    type Target = Arc<Mutex<ClientState>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// out_state 出力パラメータを廃止し、生成したポインタ自体を戻り値として返す形に変更。
pub type ClientStateNewResult = FfiResult<*const ClientStateHandle>;

#[unsafe(no_mangle)]
pub extern "C" fn client_state_new(
    callbacks: GuiCallbacks,
    std_plugin_dir: FfiStringView,
    working_dir: FfiStringView,
) -> ClientStateNewResult {
    fn inner(
        callbacks: GuiCallbacks,
        std_plugin_dir: FfiStringView,
        working_dir: FfiStringView,
    ) -> anyhow::Result<*const ClientStateHandle> {
        let std_plugin_dir = std_plugin_dir
            .as_str()
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);

        let working_dir = working_dir
            .as_str()
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);

        let dirs_def = Directories::new(std_plugin_dir, working_dir);
        let state = Arc::new(Mutex::new(ClientState::new(callbacks, dirs_def)));

        Ok(Arc::into_raw(state) as *const ClientStateHandle)
    }

    match catch_unwind(AssertUnwindSafe(|| {
        inner(callbacks, std_plugin_dir, working_dir)
    })) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn client_state_bootstrap(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        // Arc::into_raw 由来のポインタから、参照カウントを増やして
        // 独立した Arc クローンを作る（元のポインタは消費しない）
        let state = ClientStateHandle::from_ptr(ptr_state);
        let async_error = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?
            .async_error
            .clone();

        std::thread::spawn(move || {
            let async_error_inner = async_error.clone();
            let result = catch_unwind(AssertUnwindSafe(move || {
                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = async_error_inner.lock().map(|mut g| *g = Some(format!("Failed to create tokio runtime: {e}")));
                        return;
                    }
                };
                let state_arc = state.clone();

                // Phase 1: プラグインローダーとディレクトリを取得し、ClientState ロックを即座に解放
                // （ブロックの終了と同時にガードがドロップされ、ロック解放される）
                let (plugin_loader, dir) = {
                    let guard = match state_arc.lock() {
                        Ok(g) => g,
                        Err(e) => {
                            let _ = async_error_inner.lock().map(|mut g| *g = Some(format!("Failed to lock state: {e}")));
                            return;
                        }
                    };
                    (
                        Arc::clone(&guard.common.plugin_loader),
                        Directories::new(
                            guard.common.dir.std_plugin_dir.clone(),
                            guard.common.dir.working_dir.clone(),
                        ),
                    )
                };

                runtime.block_on(async {
                    // Phase 2: プラグイン読み込みを非同期で実行（ClientState ロックを保持しない）
                    match plugin_loader.write_or_err() {
                        Ok(mut loader) => {
                            match loader.load_from_disk(&dir).await {
                                Ok(_) => log::info!("Plugins loaded successfully"),
                                Err(e) => log::error!("Failed to load plugins: {:?}", e),
                            }
                        }
                        Err(e) => log::error!("Failed to load plugins: {:?}", e),
                    }

                    // Phase 3: プラグインフィールド適用（ClientState ロックを短時間保持）
                    match state_arc.lock() {
                        Ok(mut guard) => {
                            if let Err(e) = guard.apply_extra_plugin_fields() {
                                log::error!("Failed to apply settings: {:?}", e);
                            } else {
                                log::info!("Settings applied successfully");
                            }
                        }
                        Err(e) => log::error!("Failed to acquire state lock: {:?}", e),
                    }
                });
            }));

            if let Err(ref panic) = result {
                let _ = async_error.lock().map(|mut g| *g = Some(format!("bootstrap thread panicked: {:?}", panic)));
            }
            log_if_panicked(result, "Client bootstrap thread");
            // ここで state (Arc) がドロップされ、参照カウントが1つ減る
        });

        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn client_state_network_run(
    ptr_state: *const ClientStateHandle,
    addr: FfiStringView,
) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle, addr: FfiStringView) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        // Arc::into_raw 由来のポインタから、参照カウントを増やして
        // 独立した Arc クローンを作る（元のポインタは消費しない）
        let state = ClientStateHandle::from_ptr(ptr_state);
        let state_clone = Arc::clone(&state);
        let state = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
        let network = Arc::clone(&state.network);
        let async_error = state.async_error.clone();

        let addr_str = addr.as_str()?;
        let addr = addr_str.to_string();

        // Rust側でバックグラウンドスレッドを生成してワーカーを開始する
        std::thread::spawn(move || {
            let async_error_inner = async_error.clone();
            let result = catch_unwind(AssertUnwindSafe(move || {
                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = async_error_inner.lock().map(|mut g| *g = Some(format!("Failed to create tokio runtime: {e}")));
                        return;
                    }
                };
                runtime.block_on(async {
                    log::info!("Client worker thread started for: {}", addr);
                    if let Err(e) = network.run(state_clone, &addr).await {
                        let _ = async_error_inner.lock().map(|mut g| *g = Some(format!("network run error: {e}")));
                    }
                });
            }));

            if let Err(ref panic) = result {
                let _ = async_error.lock().map(|mut g| *g = Some(format!("network thread panicked: {:?}", panic)));
            }
            log_if_panicked(result, "Client worker thread");

            log::info!("Client worker thread exited");
        });

        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, addr))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_state_drop(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }
        let raw = ptr_state as *const Mutex<ClientState>;
        let _ = unsafe { Arc::from_raw(raw) };
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

/// バックグラウンドスレッドで発生した非同期エラーを取得・クリアする。
/// C++ 側は bootstrap / network_run の後でこれを呼び出し、初期化失敗を検知できる。
pub type ClientStateTakeErrorResult = FfiResultVoid;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_state_take_error(ptr_state: *const ClientStateHandle) -> ClientStateTakeErrorResult {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }
        let state = ClientStateHandle::from_ptr(ptr_state);
        let state_guard = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
        let mut error = state_guard
            .async_error
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
        if let Some(msg) = error.take() {
            return Err(anyhow::Error::msg(msg));
        }
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(())) => FfiResultVoid::ok(),
        Ok(Err(e)) => FfiResultVoid::err(e),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

// Opaque pointer for C++ - hides the actual guard type
#[repr(C)]
pub struct ProjectReadGuard {
    _private: [u8; 0], // opaque - size and alignment are flexible
}

struct ProjectReadGuardInner {
    // フィールドの宣言順に drop される。先にガードを解放し、その後で
    // RwLock 本体への最後の参照を手放すため、この順序にすること。
    _guard: RwLockReadGuard<'static, OptionProject>,
    // ガードが指す RwLock 自体を生かし続けるための所有権。
    // これが無いと、ProjectMeta 受信で state.project が差し替えられた際に
    // 旧 RwLock が解放され、C++ 側が保持中のガード経由で use-after-free
    // になる（ISSUES.md 1.1）。
    _project_arc: Arc<RwLock<OptionProject>>,
    project_ptr: *const OptionProject,
}

/// out_guard 出力パラメータを廃止し、ガードの生ポインタ自体を戻り値として返す形に変更。
pub type ProjectLockReadResult = FfiResult<*const c_void>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_state_project_lock_read(
    ptr_state: *const ClientStateHandle,
) -> ProjectLockReadResult {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<*const c_void> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state_guard = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;
        // project の Arc を先にクローンし、ガードと一緒に Box へ移す。
        // state_guard（State Mutex）はこの関数の終了で解放される。
        let project_arc = Arc::clone(&state_guard.project);
        let project_guard = project_arc.read_or_err()?;

        let project_ptr: *const Option<Project> = &*project_guard;

        // Extend lifetime to 'static - this is sound here because:
        // 1. The guard is owned by the Box and will only be freed when unlock is called
        // 2. The Box also owns an Arc clone of the RwLock the guard borrows from
        //    (`_project_arc`), so the RwLock outlives the guard even if
        //    `state.project` is replaced (e.g. by a ProjectMeta response)
        // 3. C++ side is responsible for calling unlock to free the guard
        let extended_guard = unsafe {
            std::mem::transmute::<
                RwLockReadGuard<'_, Option<Project>>,
                RwLockReadGuard<'static, Option<Project>>,
            >(project_guard)
        };

        let inner = ProjectReadGuardInner {
            _guard: extended_guard,
            _project_arc: project_arc,
            project_ptr,
        };

        let boxed_inner = Box::new(inner);
        Ok(Box::into_raw(boxed_inner) as *const c_void)
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

/// ガードをドロップ（アンロック）する関数
#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_state_project_unlock_read(
    guard_ptr: *const c_void,
) -> FfiResultVoid {
    fn inner(guard_ptr: *const c_void) -> anyhow::Result<()> {
        if guard_ptr.is_null() {
            return Err(EsotereelError::NullPointer("guard_ptr".to_string()).into());
        }
        unsafe {
            // Cast back to inner type and drop
            let inner_ptr = guard_ptr as *mut ProjectReadGuardInner;
            drop(Box::from_raw(inner_ptr));
        }
        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(guard_ptr))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

/// ガードからprojectポインタを取得する関数
/// out_project 出力パラメータを廃止し、ポインタ自体を戻り値として返す形に変更。
pub type ProjectFromGuardResult = FfiResult<*const OptionProject>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_guard_get_project_from_guard(
    guard_ptr: *const c_void,
) -> ProjectFromGuardResult {
    fn inner(guard_ptr: *const c_void) -> anyhow::Result<*const OptionProject> {
        if guard_ptr.is_null() {
            return Err(EsotereelError::NullPointer("guard_ptr".to_string()).into());
        }

        let inner = unsafe { &*(guard_ptr as *const ProjectReadGuardInner) };
        Ok(inner.project_ptr)
    }

    match catch_unwind(AssertUnwindSafe(|| inner(guard_ptr))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_state_log_directories_info(
    ptr_state: *const ClientStateHandle,
) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state
            .lock()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))?;

        state.dir.log_info();

        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}
