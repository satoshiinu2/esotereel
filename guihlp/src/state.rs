use std::sync::Arc;

use anyhow::Context;
use dashmap::DashMap;
use esotereel_lib::{
    CommonState, HostRole,
    decode::streamplayer::StreamPlayer,
    dirs::Directories,
    plugin::{script::ScriptStore, setting::SettingsStore, toolbar::ToolbarStore},
    project::ids::ResourceId,
};

use crate::network::ClientNetworkHandler;

pub struct ClientState {
    pub common: CommonState,

    pub network: Arc<ClientNetworkHandler>,

    pub stream_players: DashMap<ResourceId, StreamPlayer>,

    pub settings: SettingsStore,
    pub toolbar: ToolbarStore,
    pub scripts: ScriptStore,
}

impl ClientState {
    pub fn new(dirs_def: Directories) -> Self {
        Self {
            common: CommonState::new(dirs_def, None),
            network: Arc::new(ClientNetworkHandler::new()),
            stream_players: DashMap::new(),
            settings: SettingsStore::default(),
            toolbar: ToolbarStore::default(),
            scripts: ScriptStore::new(),
        }
    }

    pub async fn boot_strap(&mut self) {
        if let Err(e) = self.load_plugins(HostRole::Client).await {
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

    pub fn apply_plugin_fields(&mut self) -> anyhow::Result<()> {
        let (plugin_schemas, plugin_toolbar_buttons, plugin_scripts) = {
            let loader = self.plugin_loader.lock().expect("mutex poisoned");
            (
                loader.collect_all_schemas(),
                loader.collect_all_toolbars(),
                loader.collect_all_scripts(),
            )
        };

        self.settings
            .schema
            .merge_plugin_fields(plugin_schemas)
            .context("plugin schema conflict during load_plugins")?;

        self.settings.add_missing_from_schema();

        self.toolbar
            .merge_plugin_buttons(plugin_toolbar_buttons)
            .context("plugin toolbar conflict during load_plugins")?;
        self.toolbar.fill_missing_from_registry();

        self.scripts
            .merge_plugin_scripts(plugin_scripts)
            .context("plugin script conflict during load_plugins")?;

        Ok(())
    }
}

/// Provides transparent access to the shared host state.
impl std::ops::Deref for ClientState {
    type Target = CommonState;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
impl std::ops::DerefMut for ClientState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.common
    }
}
