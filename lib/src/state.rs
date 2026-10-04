use std::sync::{Arc, RwLock};

use dashmap::DashMap;

use crate::dirs::Directories;
use crate::network::NetworkHandler;
use crate::plugin::script::api::register_fn_for;
use crate::plugin::{PluginLoadedResult, PluginLoader};
use crate::project::Project;
use crate::{HostRole, StreamState};

pub struct CommonState {
    pub project: Arc<RwLock<Option<Project>>>,

    pub dir: Directories,

    /// path -> stream_states
    pub stream_state_map: Arc<DashMap<String, StreamState>>,

    /// PluginLoaderは内部サーバーで共有
    pub plugin_loader: Arc<RwLock<PluginLoader>>,

    /// rhai エンジンは内部サーバー間で共有しない
    pub script_engine: rhai::Engine,
}

impl CommonState {
    pub fn new<T: HostState>(
        dirs_def: Directories,
        shared_plugin_loader: Option<Arc<RwLock<PluginLoader>>>,
    ) -> Self {
        let plugin_loader =
            shared_plugin_loader.unwrap_or(Arc::new(RwLock::new(PluginLoader::new())));

        let mut script_engine = rhai::Engine::new();
        register_fn_for::<T>(&mut script_engine);

        Self {
            project: Arc::new(RwLock::new(None)),
            dir: dirs_def,
            stream_state_map: Arc::new(DashMap::new()),
            plugin_loader,
            script_engine,
        }
    }

    pub fn call_script<R: Clone + Send + Sync + 'static>(
        &self,
        plugin_id: &str,
        fn_name: &str,
        args: impl rhai::FuncArgs,
    ) -> anyhow::Result<R> {
        let loader = self.plugin_loader.read().expect("mutex poisoned");
        loader.call_script::<R>(&self.script_engine, plugin_id, fn_name, args)
    }

    pub async fn load_plugins(&self) -> anyhow::Result<Vec<PluginLoadedResult>> {
        let mut loader = self.plugin_loader.write().expect("mutex poisoned");
        loader.load_from_disk(&self.dir).await
    }
}

pub trait HostState: std::ops::Deref<Target = CommonState> + std::ops::DerefMut {
    type NetworkHandler: NetworkHandler;

    const ROLE: HostRole;

    async fn boot_strap(&mut self) {
        if let Err(e) = self.load_plugins().await {
            log::error!("Failed to load plugins: {:?}", e);
        } else {
            log::info!("Plugins loaded successfully");
        }

        if let Err(e) = self.apply_plugin_fields() {
            log::error!("Failed to apply settings: {:?}", e);
        } else {
            log::info!("Settings applied successfully");
        }
    }

    /// Client/Server固有の追加フィールド適用。何もしない場合はデフォルトのまま。
    fn apply_extra_plugin_fields(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn apply_plugin_fields(&mut self) -> anyhow::Result<()> {
        self.apply_extra_plugin_fields()
    }
}
