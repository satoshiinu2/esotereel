use crate::{save::autosave::autosave_loop, state::ServerState, sync::on_project_event};
use esotereel_lib::{dirs::Directories, plugin::PluginLoader, state::HostState};
use std::sync::{Arc, RwLock};

pub mod assets;
pub mod network;
pub mod project;
pub mod requests;
pub mod save;
pub mod state;
pub mod sync;

pub async fn server_network_start<F>(
    addr: &str,
    on_server_ready: Option<F>, // 起動成功したか, アドレス

    dirs_def: Directories,
    plugin_loader: Option<Arc<RwLock<PluginLoader>>>, // クライアント側から提供
) where
    F: FnOnce(bool, &str),
{
    let was_plugin_producted = plugin_loader.is_some();
    let state = ServerState::new(dirs_def, plugin_loader);

    // プラグインが提供されたものではなかったらプラグインを並列で読み込む(すでに読み込まれているので)
    if !was_plugin_producted {
        state.boot_strap().await;
    }

    // async タスク用に Clone
    let dirty_signal = Arc::clone(&state.dirty_signal);
    let network_clone = Arc::clone(&state.network);

    let state_arc = Arc::new(state);
    let state_clone = Arc::clone(&state_arc);

    // プロジェクトイベントポール用スレッド
    tokio::spawn(async move {
        loop {
            dirty_signal.notified().await;
            if let Err(e) = on_project_event(&state_clone).await {
                log::error!("Handler Error: {:?}", e);
            }
        }
    });

    let autosave_state = Arc::clone(&state_arc);
    tokio::spawn(async move {
        autosave_loop(autosave_state).await;
    });

    let state_clone = Arc::clone(&state_arc);
    if let Err(e) = network_clone.run(state_clone, addr, on_server_ready).await {
        log::error!("Server failed to start: {}", e);
    }
}
