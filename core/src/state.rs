use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicU32, Ordering},
};

use anyhow::Context;
use dashmap::DashMap;
use esotereel_lib::{
    decode::videostreamer::VideoStreamer, dirs::Directories, plugin::PluginLoader,
    project::ids::ResourceId, state::CommonState,
};
use tokio::sync::Notify;

use crate::network::ServerNetworkHandler;

pub struct ServerState {
    pub common: CommonState,

    pub network: Arc<ServerNetworkHandler>,

    pub streams: DashMap<ResourceId, VideoStreamer>,

    pub next_resource_id: AtomicU32,

    pub dirty_signal: Arc<Notify>,
}

impl ServerState {
    pub fn new(
        dirs_def: Directories,
        shared_plugin_loader: Option<Arc<RwLock<PluginLoader>>>,
    ) -> Self {
        let dirty_signal = Arc::new(Notify::new());

        Self {
            common: CommonState::new(dirs_def, shared_plugin_loader),
            network: Arc::new(ServerNetworkHandler::new(Arc::clone(&dirty_signal))),
            streams: DashMap::new(),
            next_resource_id: AtomicU32::new(0),
            dirty_signal,
        }
    }

    pub fn get_or_create_resource_id(&mut self, path: &str) -> u32 {
        self.stream_state_map
            .get(path)
            .and_then(|s| s.as_option())
            .unwrap_or_else(|| self.next_resource_id.fetch_add(1, Ordering::SeqCst))
    }
}

/// Provides transparent access to the shared host state.
impl std::ops::Deref for ServerState {
    type Target = CommonState;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
impl std::ops::DerefMut for ServerState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.common
    }
}
