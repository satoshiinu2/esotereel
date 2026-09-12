use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};

use anyhow::Context;
use dashmap::DashMap;
use esotereel_lib::{
    CommonState,
    decode::videostreamer::VideoStreamer,
    dirs::Directories,
    plugin::{PluginLoader, script::ScriptStore},
    project::ids::ResourceId,
};
use tokio::sync::Notify;

use crate::network::ServerNetworkHandler;

pub struct ServerState {
    pub common: CommonState,

    pub network: Arc<ServerNetworkHandler>,

    pub streams: DashMap<ResourceId, VideoStreamer>,

    pub next_resource_id: AtomicU32,

    pub dirty_signal: Arc<Notify>,

    pub scripts: ScriptStore,
}

impl ServerState {
    pub fn new(
        dirs_def: Directories,
        shared_plugin_loader: Option<Arc<Mutex<PluginLoader>>>,
    ) -> Self {
        let dirty_signal = Arc::new(Notify::new());

        Self {
            common: CommonState::new(dirs_def, shared_plugin_loader),
            network: Arc::new(ServerNetworkHandler::new(Arc::clone(&dirty_signal))),
            streams: DashMap::new(),
            next_resource_id: AtomicU32::new(0),
            dirty_signal,
            scripts: ScriptStore::new(),
        }
    }

    pub fn get_or_create_resource_id(&mut self, path: &str) -> u32 {
        self.path_to_stream
            .get(path)
            .and_then(|s| s.as_option())
            .unwrap_or_else(|| self.next_resource_id.fetch_add(1, Ordering::SeqCst))
    }

    pub fn apply_plugin_fields(&mut self) -> anyhow::Result<()> {
        let plugin_scripts = {
            let loader = self.plugin_loader.lock().expect("mutex poisoned");
            loader.collect_all_scripts()
        };

        self.scripts
            .merge_plugin_scripts(plugin_scripts)
            .context("plugin script conflict during load_plugins")?;

        Ok(())
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
