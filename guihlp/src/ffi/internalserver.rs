use crate::ffi::result::FfiResultVoid;
use crate::ffi::state::ClientStateHandle;
use crate::ffi::stringview::FfiStringView;
use esotereel_core::server_network_start;
use esotereel_lib::dirs::Directories;
use esotereel_lib::util::result::EsotereelError;
use std::f32::consts::E;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

pub type OnServerReadyCFn = extern "C" fn(bool, FfiStringView); // 起動成功したか

#[unsafe(no_mangle)]
pub extern "C" fn internal_server_start(
    ptr_state: *const ClientStateHandle,
    addr: FfiStringView,
    on_server_ready: OnServerReadyCFn,
    std_plugin_dir: FfiStringView,
    working_dir: FfiStringView,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        addr: FfiStringView,
        on_server_ready: OnServerReadyCFn,
        std_plugin_dir: FfiStringView,
        working_dir: FfiStringView,
    ) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_state".to_string()));
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let plugin_loader_clone = state.plugin_loader_arc();

        let addr_str = addr.as_str()?;

        let addr = addr_str.to_string();

        let std_plugin_dir = std_plugin_dir
            .as_str()
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| PathBuf::from(s));

        let working_dir = working_dir
            .as_str()
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| PathBuf::from(s));

        let dirs_def = Directories::new(std_plugin_dir, working_dir);

        std::thread::spawn(move || {
            let addr_for_panic = addr.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        // 起動失敗を GUI 側へも伝える（待ち続けさせない）
                        log::error!("Failed to create runtime: {e}");
                        on_server_ready(false, FfiStringView::from_str(addr.as_str()));
                        return;
                    }
                };

                runtime.block_on(async {
                    server_network_start(
                        addr.as_str(),
                        Some(move |result: bool, addr: &str| {
                            on_server_ready(result, FfiStringView::from_str(addr));
                        }),
                        dirs_def,
                        Some(plugin_loader_clone),
                    )
                    .await
                });
            }));

            if let Err(e) = result {
                log::error!("Server thread panicked: {:?}", e);
                on_server_ready(false, FfiStringView::from_str(addr_for_panic.as_str()));
            }
        });

        Ok(())
    }

    FfiResultVoid::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(
            ptr_state,
            addr,
            on_server_ready,
            std_plugin_dir,
            working_dir,
        )
    })))
}
