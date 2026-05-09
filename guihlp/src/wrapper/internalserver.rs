use crate::WrapperErrorCode;
use crate::wrapper::stringview::StringView;
use esotereel_core::{OnServerReadyFn, server_network_start};

#[unsafe(no_mangle)]
pub extern "C" fn internal_server_start(
    addr: StringView,
    on_server_ready: OnServerReadyFn,
) -> WrapperErrorCode {
    let Some(addr_str) = addr.as_str() else {
        return WrapperErrorCode::Error;
    };
    let addr = addr_str.to_string();

    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            runtime.block_on(async {
                server_network_start(addr.as_str(), Some(on_server_ready)).await
            });
        }));

        if let Err(e) = result {
            log::error!("Server thread panicked: {:?}", e);
        }
    });

    WrapperErrorCode::Ok
}
