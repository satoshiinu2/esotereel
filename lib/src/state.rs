use std::sync::{Arc, Mutex, RwLock};

use anyhow::Context;
use dashmap::DashMap;

use crate::dirs::Directories;
use crate::plugin::clip::ClipKindStore;
use crate::plugin::property::PropertySchema;
use crate::plugin::script::ScriptStore;
use crate::plugin::toolbar::ToolbarButtonSpec;
use crate::plugin::{PluginLoadedResult, PluginLoader};
use crate::project::Project;
use crate::{HostRole, StreamState};

pub struct CommonState {
    pub project: Option<Arc<RwLock<Project>>>,

    pub dir: Directories,

    pub path_to_stream: Arc<DashMap<String, StreamState>>,

    pub plugin_loader: Arc<Mutex<PluginLoader>>,

    pub clip_kinds: Arc<RwLock<ClipKindStore>>,

    pub scripts: Arc<RwLock<ScriptStore>>,
}

impl CommonState {
    pub fn new(
        dirs_def: Directories,
        shared_plugin_loader: Option<Arc<Mutex<PluginLoader>>>,
    ) -> Self {
        Self {
            project: None,
            dir: dirs_def,
            path_to_stream: Arc::new(DashMap::new()),
            plugin_loader: shared_plugin_loader
                .unwrap_or(Arc::new(Mutex::new(PluginLoader::new()))),
            clip_kinds: Arc::new(RwLock::new(ClipKindStore::new())),
            scripts: Arc::new(RwLock::new(ScriptStore::new())),
        }
    }

    pub async fn load_plugins(
        &mut self,
        role: HostRole,
    ) -> anyhow::Result<Vec<PluginLoadedResult>> {
        let mut loader = self.plugin_loader.lock().expect("mutex poisoned");
        loader.load_from_disk(&self.dir, role).await
    }
}

pub trait HostBootstrap: std::ops::Deref<Target = CommonState> + std::ops::DerefMut {
    const ROLE: HostRole;

    async fn boot_strap(&mut self) {
        if let Err(e) = self.load_plugins(Self::ROLE).await {
            log::error!("Failed to load plugins: {}", e);
        } else {
            log::info!("Plugins loaded successfully");
        }

        if let Err(e) = self.apply_plugin_fields() {
            log::error!("Failed to apply settings: {}", e);
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
        let (schemas, toolbar_buttons, clip_kinds, scripts) = {
            let loader = self.plugin_loader.lock().expect("mutex poisoned");
            (
                loader.collect_all_schemas(),
                loader.collect_all_toolbars(),
                loader.collect_all_clip_kinds(),
                loader.collect_all_scripts(),
            )
        };

        self.clip_kinds
            .write()
            .expect("lock poisoned")
            .merge_plugin_kinds(clip_kinds)
            .context("plugin clip kind conflict during load_plugins")?;

        self.scripts
            .write()
            .expect("lock poisoned")
            .merge_plugin_scripts(scripts)
            .context("plugin script conflict during load_plugins")?;

        self.apply_extra_plugin_fields(schemas, toolbar_buttons)
    }
}
