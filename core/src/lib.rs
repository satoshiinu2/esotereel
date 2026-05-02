use crate::network::ServerNetworkHandler;
use esotereel_lib::ServerState;
use std::sync::Arc;

pub mod network;
pub mod project;
pub mod requests;

pub async fn server_network_start(addr: &str) {
    let network = Arc::new(ServerNetworkHandler::new(Arc::new(ServerState::new())));

    if let Err(e) = network.run(addr).await {
        log::error!("Server failed to start: {}", e);
    }
}
