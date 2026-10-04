use std::{collections::BTreeMap, sync::Arc};

use dashmap::DashMap;
use esotereel_lib::{
    HostRole,
    decode::streamplayer::StreamPlayer,
    dirs::Directories,
    plugin::{
        NamespacedID,
        property::{
            PropertySchema,
            value::{FieldTypeKind, FieldValue},
        },
        settings::SettingsStore,
        toolbar::ToolbarStore,
    },
    prebootstrap::PreBootstrapSettings,
    project::ids::{ResourceId, TimelineId},
    render::video::MediaFetchCache,
    state::{CommonState, HostState},
};

use crate::{GuiCallbacks, network::ClientNetworkHandler};

pub struct ClientState {
    pub common: CommonState,

    pub network: Arc<ClientNetworkHandler>,

    pub stream_players: Arc<DashMap<ResourceId, StreamPlayer>>,

    pub media_fetch_cache: Arc<MediaFetchCache>,

    pub settings: SettingsStore,
    pub toolbar: ToolbarStore,

    pub gui_callbacks: GuiCallbacks,
}

fn core_settings_schema() -> anyhow::Result<Vec<PropertySchema>> {
    let levels = vec!["Off", "Error", "Warn", "Info", "Debug", "Trace"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let level_kind = FieldTypeKind::Enum {
        options: levels.clone(),
    };

    Ok(vec![
        PropertySchema {
            key: NamespacedID::new("core", "log.level")?,
            category: vec!["Core > Logging".to_owned()],
            label: "Default Log Level".to_owned(),
            kind: level_kind.clone(),
            default: FieldValue::Enum("Info".to_owned()),
        },
        PropertySchema {
            key: NamespacedID::new("core", "log.filters")?,
            category: vec!["Core > Logging".to_owned()],
            label: "Log ID Filters".to_owned(),
            kind: FieldTypeKind::Map {
                value_kind: Box::new(level_kind),
                known_keys: None,
            },
            default: FieldValue::Map(BTreeMap::new()),
        },
    ])
}

impl ClientState {
    pub fn new(gui_callbacks: GuiCallbacks, dirs_def: Directories) -> Self {
        let prebootstrap_settings = dirs_def
            .prebootstrap_settings_path()
            .and_then(|path| PreBootstrapSettings::load(&path))
            .unwrap_or_else(|error| {
                log::warn!("Failed to load prebootstrap settings: {error}");
                PreBootstrapSettings::default()
            });

        let mut settings = SettingsStore::from_fields(
            core_settings_schema().expect("built-in settings schema must be valid"),
        );
        settings.add_missing_from_schema();
        let log_level_key = NamespacedID::new("core", "log.level").expect("valid core settings ID");
        let log_filters_key =
            NamespacedID::new("core", "log.filters").expect("valid core settings ID");
        settings
            .set_value(
                log_level_key,
                FieldValue::Enum(prebootstrap_settings.logging.level.as_name().to_owned()),
            )
            .expect("built-in log level setting must exist");
        let log_filters = prebootstrap_settings
            .logging
            .filters
            .iter()
            .map(|(target, level)| (target.clone(), FieldValue::Enum(level.as_name().to_owned())))
            .collect();
        settings
            .set_value(log_filters_key, FieldValue::Map(log_filters))
            .expect("built-in log filters setting must exist");

        let state = Self {
            common: CommonState::new::<ClientState>(dirs_def, None),
            network: Arc::new(ClientNetworkHandler::new()),
            stream_players: Arc::new(DashMap::new()),
            settings,
            toolbar: ToolbarStore::default(),
            media_fetch_cache: Arc::new(MediaFetchCache::default()),
            gui_callbacks,
        };
        state.apply_logging_settings();
        state
    }

    pub fn mark_dirty_timeline(&self, id: TimelineId) {
        (self.gui_callbacks.mark_dirty_timeline)(id);
    }

    pub fn apply_logging_settings(&self) {
        let level_key = NamespacedID::new("core", "log.level").expect("valid core settings ID");
        let filters_key = NamespacedID::new("core", "log.filters").expect("valid core settings ID");

        if let Some(FieldValue::Enum(level) | FieldValue::String(level)) =
            self.settings.get_value(&level_key)
        {
            if let Some(level) = crate::ffi::logger::level_filter_from_name(level) {
                crate::ffi::logger::set_default_log_level(level);
            }
        }

        if let Some(FieldValue::Map(filters)) = self.settings.get_value(&filters_key) {
            let filters = filters
                .iter()
                .filter_map(|(target, value)| {
                    let level = match value {
                        FieldValue::Enum(level) | FieldValue::String(level) => level,
                        _ => return None,
                    };
                    crate::ffi::logger::level_filter_from_name(level)
                        .map(|level| (target.clone(), level))
                })
                .collect();
            crate::ffi::logger::replace_log_filters(filters);
        }
    }

    pub fn save_prebootstrap_settings(&self) -> anyhow::Result<()> {
        let mut settings = PreBootstrapSettings::default();
        let level_key = NamespacedID::new("core", "log.level")?;
        let filters_key = NamespacedID::new("core", "log.filters")?;

        if let Some(FieldValue::Enum(level) | FieldValue::String(level)) =
            self.settings.get_value(&level_key)
        {
            if let Some(level) = esotereel_lib::prebootstrap::LogLevel::parse(level) {
                settings.logging.level = level;
            }
        }
        if let Some(FieldValue::Map(filters)) = self.settings.get_value(&filters_key) {
            settings.logging.filters = filters
                .iter()
                .filter_map(|(target, value)| {
                    let name = match value {
                        FieldValue::Enum(name) | FieldValue::String(name) => name,
                        _ => return None,
                    };
                    esotereel_lib::prebootstrap::LogLevel::parse(name)
                        .map(|level| (target.clone(), level))
                })
                .collect();
        }

        let path = self.dir.prebootstrap_settings_path()?;
        settings.save(&path)
    }
}

impl HostState for ClientState {
    const ROLE: HostRole = HostRole::Client;
    type NetworkHandler = ClientNetworkHandler;

    fn apply_extra_plugin_fields(&mut self) -> anyhow::Result<()> {
        let (schemas, toolbar_buttons) = {
            let loader = self.plugin_loader.read().expect("mutex poisoned");
            (
                loader.get_settings_schemas().to_vec(),
                loader.get_toolbar_buttons().to_vec(),
            )
        };

        self.settings.merge_fields(schemas)?;
        self.settings.add_missing_from_schema();
        if let Ok(path) = self.dir.client_settings_path() {
            if let Err(error) = self.settings.load_from_path_sync(&path) {
                log::warn!("Failed to load normal settings: {error}");
            }
        }
        self.apply_logging_settings();

        self.toolbar = ToolbarStore::from_plugin_buttons(toolbar_buttons);
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
