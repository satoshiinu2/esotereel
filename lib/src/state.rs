use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use anyhow::Context;
use dashmap::DashMap;

use crate::dirs::Directories;
use crate::plugin::clip::ClipKindStore;
use crate::plugin::property::PropertySchema;
use crate::plugin::script::{CompiledScript, ScriptStore};
use crate::plugin::toolbar::ToolbarButtonSpec;
use crate::plugin::{NamespacedID, PluginLoadedResult, PluginLoader};
use crate::project::Project;
use crate::project::clip::ClipKind;
use crate::{HostRole, StreamState};

pub struct CommonState {
    pub project: Option<Arc<RwLock<Project>>>,

    pub dir: Directories,

    pub path_to_stream: Arc<DashMap<String, StreamState>>,

    pub plugin_loader: Arc<Mutex<PluginLoader>>,
    pub scripts: ScriptStore,
    pub clip_kinds: ClipKindStore,
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
            scripts: ScriptStore::new(),
            clip_kinds: ClipKindStore::new(),
        }
    }

    pub async fn load_plugins(
        &mut self,
        role: HostRole,
    ) -> anyhow::Result<Vec<PluginLoadedResult>> {
        let mut loader = self.plugin_loader.lock().expect("mutex poisoned");
        loader.load_from_disk(&self.dir, role).await
    }

    fn merge_common_plugin_fields(
        &mut self,
        plugin_scripts: HashMap<String, CompiledScript>,
        plugin_clip_kinds: HashMap<NamespacedID, ClipKind>,
    ) -> anyhow::Result<()> {
        self.scripts
            .merge_plugin_scripts(plugin_scripts)
            .context("plugin script conflict during load_plugins")?;

        self.clip_kinds
            .merge_plugin_kinds(plugin_clip_kinds)
            .context("plugin clip kind conflict during load_plugins")?;

        Ok(())
    }
}

pub trait HostBootstrap: std::ops::Deref<Target = CommonState> + std::ops::DerefMut {
    const ROLE: HostRole;

    /// Client/Server固有の追加フィールド適用。何もしない場合はデフォルトのまま。
    fn apply_extra_plugin_fields(
        &mut self,
        plugin_schemas: Vec<PropertySchema>,
        plugin_toolbar_buttons: Vec<(String, ToolbarButtonSpec)>,
    ) -> anyhow::Result<()> {
        let _ = (plugin_schemas, plugin_toolbar_buttons);
        Ok(())
    }

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

    fn apply_plugin_fields(&mut self) -> anyhow::Result<()> {
        let (plugin_schemas, plugin_clip_kinds, plugin_toolbar_buttons, plugin_scripts) = {
            let loader = self.plugin_loader.lock().expect("mutex poisoned");
            (
                loader.collect_all_schemas(),
                loader.collect_all_clip_kinds(),
                loader.collect_all_toolbars(),
                loader.collect_all_scripts(),
            )
        };

        self.merge_common_plugin_fields(plugin_scripts, plugin_clip_kinds)?;
        self.apply_extra_plugin_fields(plugin_schemas, plugin_toolbar_buttons)
    }
}
