use esotereel_lib::ClientState;

use crate::{WrapperErrorCode, network::ClientNetworkHandler, wrapper::stringview::StringView};
use std::sync::Arc;

#[unsafe(no_mangle)]
pub extern "C" fn client_network_handler_run(
    ptr: *const ClientNetworkHandler,
    addr: StringView,
) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::NullPtr;
    }

    let network_arc = unsafe { Arc::from_raw(ptr) };
    let network = Arc::clone(&network_arc);
    // 重要: instance_arc の所有権を解放せずに生ポインタに戻す
    let _ = Arc::into_raw(network_arc);

    let Some(addr_str) = addr.as_str() else {
        return WrapperErrorCode::Error;
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

        if let Err(err) = result {
            // パニックの内容をログに出力（Any型なので型情報は制限されるが、発生事実は記録できる）
            log::error!("Client worker thread panicked: {:?}", err);
        }

        log::info!("Client worker thread exited");
    });

    WrapperErrorCode::Ok
}
#[unsafe(no_mangle)]
pub extern "C" fn client_network_handler_new(
    out: *mut *const ClientNetworkHandler,
) -> WrapperErrorCode {
    if out.is_null() {
        return WrapperErrorCode::NullPtr;
    }

    let network = ClientNetworkHandler::new(Arc::new(ClientState::new()));

    unsafe {
        *out = Arc::into_raw(Arc::new(network));
    }

    WrapperErrorCode::Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn client_network_handler_drop(
    ptr: *const ClientNetworkHandler,
) -> WrapperErrorCode {
    if ptr.is_null() {
        return WrapperErrorCode::NullPtr;
    }

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = unsafe { Arc::from_raw(ptr) };
        WrapperErrorCode::Ok
    }))
    .unwrap_or(WrapperErrorCode::Error)
}
