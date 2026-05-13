use crate::network::ServerNetworkHandler;
use esotereel_lib::ServerState;
use std::sync::{Arc, Mutex};

pub mod network;
pub mod project;
pub mod requests;

pub type OnServerReadyFn = extern "C" fn(bool); // 起動成功したか

pub async fn server_network_start(addr: &str, on_server_ready: Option<OnServerReadyFn>) {
    let network = Arc::new(ServerNetworkHandler::new(Arc::new(Mutex::new(
        ServerState::new(),
    ))));

    if let Err(e) = network.run(addr, on_server_ready).await {
        log::error!("Server failed to start: {}", e);
    }
}
