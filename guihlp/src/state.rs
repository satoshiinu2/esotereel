use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use dashmap::DashMap;
use esotereel_lib::{
    HostRole,
    decode::streamplayer::StreamPlayer,
    dirs::Directories,
    plugin::{
        NamespacedID,
        property::PropertySchema,
        script::ScriptStore,
        settings::SettingsStore,
        toolbar::{ToolbarButtonSpec, ToolbarStore},
    },
    project::{clip::ClipKind, ids::ResourceId},
    render::video::{MediaFetchCache, builder::VertexBatch},
    state::{CommonState, HostBootstrap},
};

use crate::network::ClientNetworkHandler;

pub struct ClientState {
    pub common: CommonState,

    pub network: Arc<ClientNetworkHandler>,

    pub stream_players: Arc<DashMap<ResourceId, StreamPlayer>>,

    pub media_fetch_cache: Arc<MediaFetchCache>,

    pub settings: SettingsStore,
    pub toolbar: ToolbarStore,
}

impl ClientState {
    pub fn new(dirs_def: Directories) -> Self {
        Self {
            common: CommonState::new(dirs_def, None),
            network: Arc::new(ClientNetworkHandler::new()),
            stream_players: Arc::new(DashMap::new()),
            settings: SettingsStore::default(),
            toolbar: ToolbarStore::default(),
            media_fetch_cache: Arc::new(MediaFetchCache::default()),
        }
    }
}

impl HostBootstrap for ClientState {
    const ROLE: HostRole = HostRole::Client;

    fn apply_extra_plugin_fields(
        &mut self,
        plugin_schemas: Vec<PropertySchema>,
        plugin_toolbar_buttons: Vec<(String, ToolbarButtonSpec)>,
    ) -> anyhow::Result<()> {
        self.settings
            .schema
            .merge_plugin_fields(plugin_schemas)
            .context("plugin schema conflict during load_plugins")?;
        self.settings.add_missing_from_schema();

        self.toolbar
            .merge_plugin_buttons(plugin_toolbar_buttons)
            .context("plugin toolbar conflict during load_plugins")?;
        self.toolbar.fill_missing_from_registry();

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
