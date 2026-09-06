use crate::WrapperErrorCode;
use crate::ffi::stringview::StringView;
use esotereel_core::{OnServerReadyFn, server_network_start};
use esotereel_lib::dirs::Directories;
use std::path::PathBuf;

#[unsafe(no_mangle)]
pub extern "C" fn internal_server_start(
    addr: StringView,
    on_server_ready: OnServerReadyFn,
    std_plugin_dir: StringView,
    working_dir: StringView,
) -> WrapperErrorCode {
    let Some(addr_str) = addr.as_str() else {
        return WrapperErrorCode::invalid_string_error();
    };
    let addr = addr_str.to_string();

    let std_plugin_dir = std_plugin_dir
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(s));

    let working_dir = working_dir
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(s));

    let dirs_def = Directories::new(std_plugin_dir, working_dir);

    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");

            runtime.block_on(async {
                server_network_start(addr.as_str(), Some(on_server_ready), dirs_def).await
            });
        }));

        if let Err(e) = result {
            log::error!("Server thread panicked: {:?}", e);
        }
    });

    WrapperErrorCode::ok()
}
