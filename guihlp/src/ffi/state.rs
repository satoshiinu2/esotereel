use esotereel_lib::{dirs::Directories, project::Project};

use crate::{
    WrapperErrorCode,
    ffi::{log_if_panicked, stringview::StringView},
    state::ClientState,
};
use std::{
    ffi::c_void,
    path::PathBuf,
    sync::{Arc, Mutex, RwLockReadGuard},
};

pub struct ClientStateHandle(pub Arc<Mutex<ClientState>>);

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

#[unsafe(no_mangle)]
pub extern "C" fn client_state_new(
    out_state: *mut *const ClientStateHandle,
    std_plugin_dir: StringView,
    working_dir: StringView,
) -> WrapperErrorCode {
    if out_state.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let std_plugin_dir = std_plugin_dir
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(s));

    let working_dir = working_dir
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(s));

    let dirs_def = Directories::new(std_plugin_dir, working_dir);

    let state = Arc::new(Mutex::new(ClientState::new(dirs_def)));

    unsafe {
        *out_state = Arc::into_raw(state) as *const ClientStateHandle;
    }

    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn client_state_bootstrap(ptr_state: *const ClientStateHandle) -> WrapperErrorCode {
    if ptr_state.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    // Arc::into_raw 由来のポインタから、参照カウントを増やして
    // 独立した Arc クローンを作る（元のポインタは消費しない）
    let state = ClientStateHandle::from_ptr(ptr_state);

    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(async {
                let mut state = state.lock().expect("mutex poisoned");
                state.boot_strap().await;
            });
        }));

        log_if_panicked(result, "Client bootstrap thread");
        // ここで state (Arc) がドロップされ、参照カウントが1つ減る
    });
    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn client_state_network_run(
    ptr_state: *const ClientStateHandle,
    addr: StringView,
) -> WrapperErrorCode {
    if ptr_state.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    // Arc::into_raw 由来のポインタから、参照カウントを増やして
    // 独立した Arc クローンを作る（元のポインタは消費しない）
    let state = ClientStateHandle::from_ptr(ptr_state);
    let state_clone = Arc::clone(&state);
    let state = state.lock().expect("mutex poisoned");
    let network = Arc::clone(&state.network);

    let Some(addr_str) = addr.as_str() else {
        return WrapperErrorCode::invalid_string_error();
    };
    let addr = addr_str.to_string();

    // Rust側でバックグラウンドスレッドを生成してワーカーを開始する
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(async {
                log::info!("Client worker thread started for: {}", addr);

                if let Err(e) = network.run(state_clone, &addr).await {
                    log::error!("Client worker error: {}", e);
                }
            });
        }));

        log_if_panicked(result, "Client worker thread");

        log::info!("Client worker thread exited");
    });

    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn client_state_drop(ptr_state: *const ClientStateHandle) -> WrapperErrorCode {
    if ptr_state.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ptr = ptr_state as *const ClientStateHandle;
        let _ = unsafe { Arc::from_raw(ptr) };
        WrapperErrorCode::ok()
    }));

    let msg = log_if_panicked(result, "client_state_drop");

    match msg {
        Some(m) => WrapperErrorCode::panic(Some(&m)),
        None => WrapperErrorCode::ok(),
    }
}

// Opaque pointer for C++ - hides the actual guard type
#[repr(C)]
pub struct ProjectReadGuard {
    _private: [u8; 0], // opaque - size and alignment are flexible
}

struct ProjectReadGuardInner {
    _guard: RwLockReadGuard<'static, Project>,
    project_ptr: *const Project,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_state_project_lock_read(
    ptr_state: *const ClientStateHandle,
    out_guard: *mut *const c_void,
) -> WrapperErrorCode {
    if ptr_state.is_null() || out_guard.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let state = ClientStateHandle::from_ptr(ptr_state);

    let Ok(app_state) = state.lock() else {
        return WrapperErrorCode::panic(Some("mutex poisoned"));
    };

    let Some(project_arc) = app_state.project.as_ref() else {
        return WrapperErrorCode::not_found(Some("project not found"));
    };

    // Use try_read to avoid blocking the UI thread - if lock is not immediately available, return error
    let lock = match project_arc.try_read() {
        Ok(guard) => guard,
        Err(_) => return WrapperErrorCode::error(Some("lock busy - retry later")),
    };

    let project_ptr: *const Project = &*lock;

    // Extend lifetime to 'static - this is safe because:
    // 1. The guard is owned by the Box and will only be freed when unlock is called
    // 2. The project_ptr remains valid as long as the guard is held
    // 3. C++ side is responsible for calling unlock to free the guard
    let extended_guard = unsafe {
        std::mem::transmute::<RwLockReadGuard<'_, Project>, RwLockReadGuard<'static, Project>>(lock)
    };

    let inner = ProjectReadGuardInner {
        _guard: extended_guard,
        project_ptr,
    };

    // Box the inner struct and cast to opaque pointer
    let boxed_inner = Box::new(inner);
    let opaque_ptr = Box::into_raw(boxed_inner) as *const c_void;

    unsafe {
        *out_guard = opaque_ptr;
    }

    WrapperErrorCode::ok()
}

/// ガードをドロップ（アンロック）する関数
#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_state_project_unlock_read(
    guard_ptr: *const c_void,
) -> WrapperErrorCode {
    if !guard_ptr.is_null() {
        unsafe {
            // Cast back to inner type and drop
            let inner_ptr = guard_ptr as *mut ProjectReadGuardInner;
            drop(Box::from_raw(inner_ptr));
        }
        WrapperErrorCode::ok()
    } else {
        WrapperErrorCode::null_ptr()
    }
}

/// ガードからprojectポインタを取得する関数
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_guard_get_project_from_guard(
    guard_ptr: *const c_void,
    out_project: *mut *const Project,
) -> WrapperErrorCode {
    if guard_ptr.is_null() || out_project.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    unsafe {
        let inner = &*(guard_ptr as *const ProjectReadGuardInner);
        *out_project = inner.project_ptr;
    }

    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_state_log_directories_info(
    ptr_state: *const ClientStateHandle,
) -> WrapperErrorCode {
    if ptr_state.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let state = ClientStateHandle::from_ptr(ptr_state);

    let state = state.lock().expect("mutex poisoned");

    state.dir.log_info();

    WrapperErrorCode::ok()
}
