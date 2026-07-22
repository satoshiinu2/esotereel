use esotereel_lib::{ClientState, project::Project};

use crate::{
    WrapperErrorCode,
    network::ClientNetworkHandler,
    wrapper::{log_if_panicked, stringview::StringView},
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_network_handler_app_state_project_lock_read(
    ptr: *const ClientNetworkHandler,
    out: *mut *const c_void,
) -> WrapperErrorCode {
    if ptr.is_null() || out.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let handler = unsafe { &*ptr };
    // ガード自体をヒープに置いて、その「鍵」を返す
    let lock = handler.app_state.lock().expect("mutex poisoned");
    let lock = lock.project.read().unwrap();
    if lock.as_ref().is_some() {
        // ロックを維持するためにガードを leak させる
        unsafe { *out = Box::into_raw(Box::new(lock)) as *const c_void };
        WrapperErrorCode::ok()
    } else {
        unsafe { *out = std::ptr::null_mut() as *const c_void };
        WrapperErrorCode::not_found(Some("project not found"))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_network_handler_app_state_project_unlock_read(
    guard_ptr: *const c_void,
) -> WrapperErrorCode {
    if !guard_ptr.is_null() {
        // leak させた Box を戻してドロップ
        unsafe {
            drop(Box::from_raw(
                guard_ptr as *mut RwLockReadGuard<Option<Arc<Project>>>,
            ))
        };
        WrapperErrorCode::ok()
    } else {
        WrapperErrorCode::null_ptr()
    }
}
