use std::sync::{Arc, RwLock};

use dashmap::DashMap;

use crate::dirs::Directories;
use crate::plugin::property::PropertySchema;
use crate::plugin::toolbar::ToolbarButtonSpec;
use crate::plugin::{PluginLoadedResult, PluginLoader};
use crate::project::Project;
use crate::{HostRole, StreamState};

pub struct CommonState {
    pub project: Arc<RwLock<Option<Project>>>,

    pub dir: Directories,

    /// path -> stream_states
    pub stream_state_map: Arc<DashMap<String, StreamState>>,

    pub plugin_loader: Arc<RwLock<PluginLoader>>,
}

impl CommonState {
    pub fn new(
        dirs_def: Directories,
        shared_plugin_loader: Option<Arc<RwLock<PluginLoader>>>,
    ) -> Self {
        Self {
            project: Arc::new(RwLock::new(None)),
            dir: dirs_def,
            stream_state_map: Arc::new(DashMap::new()),
            plugin_loader: shared_plugin_loader
                .unwrap_or(Arc::new(RwLock::new(PluginLoader::new()))),
        }
    }

    pub async fn load_plugins(&self, role: HostRole) -> anyhow::Result<Vec<PluginLoadedResult>> {
        let mut loader = self.plugin_loader.write().expect("mutex poisoned");
        loader.load_from_disk(&self.dir, role).await
    }
}

pub trait HostBootstrap: std::ops::Deref<Target = CommonState> + std::ops::DerefMut {
    const ROLE: HostRole;

    async fn boot_strap(&mut self) {
        if let Err(e) = self.load_plugins(Self::ROLE).await {
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
    fn apply_extra_plugin_fields(
        &mut self,
        plugin_schemas: Vec<PropertySchema>,
        plugin_toolbar_buttons: Vec<(String, ToolbarButtonSpec)>,
    ) -> anyhow::Result<()> {
        let _ = (plugin_schemas, plugin_toolbar_buttons);
        Ok(())
    }

    fn apply_plugin_fields(&mut self) -> anyhow::Result<()> {
        let (schemas, toolbar_buttons) = {
            let loader = self.plugin_loader.read().expect("mutex poisoned");
            (loader.collect_all_schemas(), loader.collect_all_toolbars())
        };

        self.apply_extra_plugin_fields(schemas, toolbar_buttons)
    }
}
