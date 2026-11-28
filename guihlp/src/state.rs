use std::{
    collections::{BTreeMap, HashSet},
    ops::Range,
    sync::{Arc, Mutex, RwLock},
};

use dashmap::DashMap;
use esotereel_lib::{
    decode::streamplayer::StreamPlayer,
    dirs::Directories,
    plugin::{
        property::{
            value::{FieldTypeKind, FieldValue},
            PropertyKey, PropertySchema,
        },
        settings::SettingsStore,
        toolbar::ToolbarStore,
        NamespacedID, PluginLoader,
    },
    prebootstrap::PreBootstrapSettings,
    project::{
        asset::AssetEntry,
        autosave::AutosaveGeneration,
        ids::{AssetRef, ResourceId, TimelineId},
        save::VersionStatus,
        MediaSec, Project, TimelineTick,
    },
    render::video::MediaFetchCache,
    state::{CommonState, HostState},
    util::result::LockExt,
    HostRole,
};

use crate::{network::ClientNetworkHandler, GuiCallbacks};

pub struct ClientState {
    pub common: CommonState,

    pub network: Arc<ClientNetworkHandler>,

    pub stream_players: Arc<DashMap<ResourceId, StreamPlayer>>,

    /// 直前の FetchClipsInRange 要求: timeline_id => (start, end)。
    /// 同一タイムラインへの同一レンジの重複送信を抑制するために使用する。
    pub last_clips_fetch: DashMap<TimelineId, (TimelineTick, TimelineTick)>,

    /// 直前の FetchStreamData 要求: resource_id => 要求済みレンジのリスト。
    /// 同一リソースへの同一レンジの重複送信を抑制するために使用する。
    pub last_stream_fetch: DashMap<ResourceId, Vec<Range<MediaSec>>>,

    pub media_fetch_cache: Arc<MediaFetchCache>,

    /// 設定ストア。読み取りが支配的で書き込みは設定変更時のみのため、
    /// ストア単位の `RwLock` で保護し、ClientState 全体のロックは持たない。
    pub settings: Arc<RwLock<SettingsStore>>,
    /// ツールバーストア。settings と同じくストア単位の `RwLock` で保護する。
    pub toolbar: Arc<RwLock<ToolbarStore>>,

    pub gui_callbacks: GuiCallbacks,

    /// バックグラウンドスレッド（bootstrap / network_run）で発生した非同期エラーを格納。
    /// C++ 側は `client_state_take_error` で取得・クリアできる。
    pub async_error: Arc<Mutex<Option<String>>>,

    /// プロジェクトフォルダの事前確認（バージョン判定）結果。
    /// C++ 側は FFI で取り出して段階表示に使う。
    pub project_inspection: Arc<Mutex<Option<ProjectInspection>>>,

    /// プロジェクトを開いた結果。
    pub project_opened: Arc<Mutex<Option<ProjectOpenedInfo>>>,

    /// 最後に保存が完了したプロジェクトフォルダ。
    pub project_saved: Arc<Mutex<Option<String>>>,

    /// サーバーから届いたオートセーブ世代の一覧。
    pub autosave_generations: Arc<Mutex<Vec<AutosaveGeneration>>>,

    /// サーバーから同期されたアセット表（表示順）。全量配信で
    /// 置き換わり、増分配信で更新される。GUI のアセット一覧表示用。
    pub synced_assets: Arc<Mutex<Vec<AssetEntry>>>,

    /// 同期されたアセットのうち、クリップから使用中のものの ID 集合。
    /// 使用判定はサーバー側の走査結果が配信に同梱されるため、
    /// エントリ本体とは別に保持します。
    pub synced_assets_in_use: Arc<Mutex<HashSet<AssetRef>>>,

    /// サーバーが現在開いているプロジェクトフォルダ（開く／保存の
    /// 応答で更新される）。プロジェクト設定タブの編集先に使う。
    pub current_project_folder: Arc<RwLock<Option<String>>>,

    /// 前回セッションが異常終了したかどうか（正常終了マーカー不在）。
    pub previous_session_abnormal: bool,

    /// 正常終了マーカーのパス。正常終了時に消す。
    session_marker: Option<std::path::PathBuf>,
}

/// プロジェクトフォルダの事前確認結果です。
#[derive(Debug, Clone)]
pub struct ProjectInspection {
    pub path: String,
    pub version_major: u32,
    pub version_minor: u32,
    pub version_revision: u32,
    pub status: VersionStatus,
    pub project_id: String,
}

/// プロジェクトを開いた結果です。
#[derive(Debug, Clone)]
pub struct ProjectOpenedInfo {
    pub path: String,
    pub version_major: u32,
    pub version_minor: u32,
    pub version_revision: u32,
    pub status: VersionStatus,
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
            key: PropertyKey::from_namespaced(NamespacedID::new("core", "log.level")?),
            category: vec!["Core > Logging".to_owned()],
            label: "Default Log Level".to_owned(),
            kind: level_kind.clone(),
            default: FieldValue::Enum("Info".to_owned()),
            capability: None,
        },
        PropertySchema {
            key: PropertyKey::from_namespaced(NamespacedID::new("core", "log.filters")?),
            category: vec!["Core > Logging".to_owned()],
            label: "Log ID Filters".to_owned(),
            kind: FieldTypeKind::Map {
                value_kind: Box::new(level_kind),
                known_keys: None,
            },
            default: FieldValue::Map(BTreeMap::new()),
            capability: None,
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

        // 異常終了の検知: 正常終了マーカーが残っていれば前回は異常終了。
        // 検知したら今回分のマーカーを置き直す（正常終了時に消す）。
        let session_marker = dirs_def.data_dir().ok().map(|dir| dir.join("session.marker"));
        let previous_session_abnormal = session_marker
            .as_ref()
            .is_some_and(|marker| marker.exists());
        if previous_session_abnormal {
            log::warn!("previous session did not exit cleanly (session marker remained)");
        }
        if let Some(marker) = &session_marker {
            if let Some(parent) = marker.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(error) = std::fs::write(marker, b"running") {
                log::warn!("failed to write session marker {}: {error}", marker.display());
            }
        }

        let state = Self {
            common: CommonState::new::<ClientState>(dirs_def, None),
            network: Arc::new(ClientNetworkHandler::new()),
            stream_players: Arc::new(DashMap::new()),
            last_clips_fetch: DashMap::new(),
            last_stream_fetch: DashMap::new(),
            settings: Arc::new(RwLock::new(settings)),
            toolbar: Arc::new(RwLock::new(ToolbarStore::default())),
            media_fetch_cache: Arc::new(MediaFetchCache::default()),
            gui_callbacks,
            async_error: Arc::new(Mutex::new(None)),
            project_inspection: Arc::new(Mutex::new(None)),
            project_opened: Arc::new(Mutex::new(None)),
            project_saved: Arc::new(Mutex::new(None)),
            autosave_generations: Arc::new(Mutex::new(Vec::new())),
            synced_assets: Arc::new(Mutex::new(Vec::new())),
            synced_assets_in_use: Arc::new(Mutex::new(HashSet::new())),
            current_project_folder: Arc::new(RwLock::new(None)),
            previous_session_abnormal,
            session_marker,
        };
        state.apply_logging_settings();
        state
    }

    /// 正常終了を記録します（マーカーを消す）。次回起動時にマーカーが
    /// 残っていなければ異常終了とは判定されません。
    pub fn mark_clean_exit(&self) {
        if let Some(marker) = &self.session_marker {
            if let Err(error) = std::fs::remove_file(marker) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!(
                        "failed to remove session marker {}: {error}",
                        marker.display()
                    );
                }
            }
        }
    }

    pub fn apply_logging_settings(&self) {
        let level_key = NamespacedID::new("core", "log.level").expect("valid core settings ID");
        let filters_key = NamespacedID::new("core", "log.filters").expect("valid core settings ID");

        let Ok(settings) = self.settings.read_or_err() else {
            return;
        };

        if let Some(FieldValue::Enum(level) | FieldValue::String(level)) =
            settings.get_value(&level_key)
        {
            if let Some(level) = crate::logger::level_filter_from_name(level) {
                crate::logger::set_default_log_level(level);
            }
        }

        if let Some(FieldValue::Map(filters)) = settings.get_value(&filters_key) {
            let filters = filters
                .iter()
                .filter_map(|(target, value)| {
                    let level = match value {
                        FieldValue::Enum(level) | FieldValue::String(level) => level,
                        _ => return None,
                    };
                    crate::logger::level_filter_from_name(level)
                        .map(|level| (target.clone(), level))
                })
                .collect();
            crate::logger::replace_log_filters(filters);
        }
    }

    pub fn save_prebootstrap_settings(&self) -> anyhow::Result<()> {
        let mut settings = PreBootstrapSettings::default();
        let level_key = NamespacedID::new("core", "log.level")?;
        let filters_key = NamespacedID::new("core", "log.filters")?;

        let store = self.settings.read_or_err()?;

        if let Some(FieldValue::Enum(level) | FieldValue::String(level)) =
            store.get_value(&level_key)
        {
            if let Some(level) = esotereel_lib::prebootstrap::LogLevel::parse(level) {
                settings.logging.level = level;
            }
        }
        if let Some(FieldValue::Map(filters)) = store.get_value(&filters_key) {
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

    // メンバー取得アクセサ群。FFI やレスポンス処理は ClientState 全体を
    // ロックせず、必要なメンバーを `Arc` で取り出してから処理する。

    pub fn project_arc(&self) -> Arc<RwLock<Option<Project>>> {
        Arc::clone(&self.common.project)
    }

    pub fn network_arc(&self) -> Arc<ClientNetworkHandler> {
        Arc::clone(&self.network)
    }

    pub fn plugin_loader_arc(&self) -> Arc<RwLock<PluginLoader>> {
        Arc::clone(&self.common.plugin_loader)
    }

    pub fn stream_players_arc(&self) -> Arc<DashMap<ResourceId, StreamPlayer>> {
        Arc::clone(&self.stream_players)
    }

    pub fn stream_state_map_arc(&self) -> Arc<DashMap<String, esotereel_lib::StreamState>> {
        Arc::clone(&self.common.stream_state_map)
    }

    pub fn media_fetch_cache_arc(&self) -> Arc<MediaFetchCache> {
        Arc::clone(&self.media_fetch_cache)
    }

    pub fn settings_arc(&self) -> Arc<RwLock<SettingsStore>> {
        Arc::clone(&self.settings)
    }

    pub fn toolbar_arc(&self) -> Arc<RwLock<ToolbarStore>> {
        Arc::clone(&self.toolbar)
    }

    pub fn async_error_arc(&self) -> Arc<Mutex<Option<String>>> {
        Arc::clone(&self.async_error)
    }
}

impl HostState for ClientState {
    const ROLE: HostRole = HostRole::Client;
    type NetworkHandler = ClientNetworkHandler;

    fn apply_extra_plugin_fields(&self) -> anyhow::Result<()> {
        let (schemas, toolbar_buttons) = {
            let loader = self.plugin_loader.read_or_err()?;
            (
                loader.get_settings_schemas().to_vec(),
                loader.get_toolbar_buttons().to_vec(),
            )
        };

        {
            let mut settings = self.settings.write_or_err()?;
            settings.merge_fields(schemas)?;
            settings.add_missing_from_schema();
            if let Ok(path) = self.dir.client_settings_path() {
                if let Err(error) = settings.load_from_path_sync(&path) {
                    log::warn!("Failed to load normal settings: {error}");
                }
            }
        } // settings の write ガードはここで解放（apply_logging_settings が read を取るため）
        self.apply_logging_settings();

        {
            let mut toolbar = self.toolbar.write_or_err()?;
            *toolbar = ToolbarStore::from_plugin_buttons(toolbar_buttons);
            toolbar.fill_missing_from_registry();
        }

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
