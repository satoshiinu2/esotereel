use crate::WrapperErrorCode;
use crate::ffi::stringview::StringView;
use crate::network::ClientNetworkHandler;
use esotereel_core::server_network_start;
use esotereel_lib::dirs::Directories;
use std::path::PathBuf;
use std::sync::Arc;

pub type OnServerReadyCFn = extern "C" fn(bool, StringView); // 起動成功したか

#[unsafe(no_mangle)]
pub extern "C" fn internal_server_start(
    network_ptr: *const ClientNetworkHandler,
    addr: StringView,
    on_server_ready: OnServerReadyCFn,
    std_plugin_dir: StringView,
    working_dir: StringView,
) -> WrapperErrorCode {
    if network_ptr.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network_arc = unsafe { Arc::from_raw(network_ptr) };
    let network = Arc::clone(&network_arc);
    // 重要: instance_arc の所有権を解放せずに生ポインタに戻す
    let _ = Arc::into_raw(network_arc);

    let plugin_loader_clone = network
        .app_state
        .lock()
        .expect("mutex poisoned")
        .plugin_loader
        .clone();

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
                server_network_start(
                    addr.as_str(),
                    Some(move |result: bool, addr: &str| {
                        on_server_ready(result, StringView::from_str(addr));
                    }),
                    dirs_def,
                    Some(plugin_loader_clone),
                )
                .await
            });
        }));

        if let Err(e) = result {
            log::error!("Server thread panicked: {:?}", e);
        }
    });

    WrapperErrorCode::ok()
}
