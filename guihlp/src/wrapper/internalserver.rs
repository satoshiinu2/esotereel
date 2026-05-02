use crate::WrapperErrorCode;
use crate::wrapper::stringview::StringView;
use esotereel_lib::ServerState;
use std::sync::Arc;

use esotereel_core::network::ServerNetworkHandler;

#[unsafe(no_mangle)]
pub extern "C" fn internal_server_start(addr: StringView) -> WrapperErrorCode {
    let Some(addr_str) = addr.as_str() else {
        return WrapperErrorCode::Error;
    };
    let addr = addr_str.to_string();

    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            runtime.block_on(async {
                let server = Arc::new(ServerNetworkHandler::new(Arc::new(ServerState::new())));
                log::info!("Server: Starting in-process server on {}", addr);
                if let Err(e) = server.run(&addr).await {
                    log::error!("Server: In-process server error: {}", e);
                }
            });
        }));

        if let Err(e) = result {
            log::error!("Server thread panicked: {:?}", e);
        }
    });

    WrapperErrorCode::Ok
}
