use esotereel_lib::{ClientState, project::Project};

use crate::{
    WrapperErrorCode,
    ffi::{log_if_panicked, stringview::StringView},
    network::ClientNetworkHandler,
};
use std::{
    ffi::c_void,
    sync::{Arc, Mutex, RwLockReadGuard},
};

#[unsafe(no_mangle)]
pub extern "C" fn client_network_handler_run(
    ptr: *const ClientNetworkHandler,
    addr: StringView,
) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network_arc = unsafe { Arc::from_raw(ptr) };
    let network = Arc::clone(&network_arc);
    // 重要: instance_arc の所有権を解放せずに生ポインタに戻す
    let _ = Arc::into_raw(network_arc);

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
                
                // プラグインを並列で読み込む
                {
                    let mut app_state = network.app_state.lock().expect("mutex poisoned");
                    if let Err(e) = app_state.load_plugins().await {
                        log::error!("Failed to load plugins: {}", e);
                    } else {
                        log::info!("Plugins loaded successfully");
                    }
                }
                
                if let Err(e) = network.run(&addr).await {
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
pub extern "C" fn client_network_handler_new(
    out: *mut *const ClientNetworkHandler,
) -> WrapperErrorCode {
    if out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = ClientNetworkHandler::new(Arc::new(Mutex::new(ClientState::new())));

    unsafe {
        *out = Arc::into_raw(Arc::new(network));
    }

    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn client_network_handler_drop(
    ptr: *const ClientNetworkHandler,
) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = unsafe { Arc::from_raw(ptr) };
        WrapperErrorCode::ok()
    }));

    let msg = log_if_panicked(result, "client_network_handler_drop");
    WrapperErrorCode::panic(msg.as_deref())
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
pub unsafe extern "C" fn client_network_handler_app_state_project_lock_read(
    ptr: *const ClientNetworkHandler,
    out_guard: *mut *const c_void,
) -> WrapperErrorCode {
    if ptr.is_null() || out_guard.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr };

    let Ok(app_state) = network.app_state.lock() else {
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
pub unsafe extern "C" fn client_network_handler_app_state_project_unlock_read(
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
