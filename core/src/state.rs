use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex, RwLock,
    },
};

use dashmap::DashMap;
use esotereel_lib::{
    decode::videostreamer::VideoStreamer,
    dirs::Directories,
    plugin::PluginLoader,
    project::asset::AssetEntry,
    project::ids::AssetRef,
    project::ids::ResourceId,
    project::ids::TimelineId,
    state::{CommonState, HostState},
    HostRole,
};
use tokio::sync::Notify;

use crate::assets::UploadManager;
use crate::network::ServerNetworkHandler;
use crate::project::history::HistoryStack;

pub struct ServerState {
    pub common: CommonState,

    pub network: Arc<ServerNetworkHandler>,

    pub streams: DashMap<ResourceId, VideoStreamer>,

    pub histories: Arc<Mutex<HashMap<TimelineId, HistoryStack>>>,

    pub next_resource_id: AtomicU32,

    pub dirty_signal: Arc<Notify>,

    pub uploads: Arc<Mutex<UploadManager>>,

    /// 現在開いているプロジェクトフォルダです。新規プロジェクトは
    /// `data_dir/temp/<プロジェクト ID>` の仮フォルダ、名前付き保存後は
    /// 実フォルダになります。
    pub project_folder: Arc<RwLock<Option<PathBuf>>>,

    /// オートセーブすべき変更が溜まっているかどうかです。
    /// 変更イベント（`on_project_event`）で立て、世代を書けたら下ろします。
    pub project_dirty: Arc<AtomicBool>,

    /// クライアントへ配信済みの kind 表の長さです。新規採番分だけを
    /// `KindTableAppended` で配るために使います。
    pub kind_sync_len: Arc<Mutex<usize>>,

    /// クライアントへ配信済みのアセット表（アセット ID → エントリと
    /// 使用中フラグ）です。現在のレジストリとの差分だけを
    /// `AssetTableUpdated` で配るために使います。プロジェクトが
    /// 差し替わったら全量配信側で作り直します。
    pub asset_sync_snapshot: Arc<Mutex<HashMap<AssetRef, (AssetEntry, bool)>>>,
}

impl ServerState {
    pub fn new(
        dirs_def: Directories,
        shared_plugin_loader: Option<Arc<RwLock<PluginLoader>>>,
    ) -> Self {
        let dirty_signal = Arc::new(Notify::new());

        Self {
            common: CommonState::new::<ServerState>(dirs_def.clone(), shared_plugin_loader),
            network: Arc::new(ServerNetworkHandler::new(Arc::clone(&dirty_signal))),
            streams: DashMap::new(),
            histories: Arc::new(Mutex::new(HashMap::new())),
            next_resource_id: AtomicU32::new(0),
            dirty_signal,
            uploads: Arc::new(Mutex::new(UploadManager::new(&dirs_def))),
            project_folder: Arc::new(RwLock::new(None)),
            project_dirty: Arc::new(AtomicBool::new(false)),
            kind_sync_len: Arc::new(Mutex::new(0)),
            asset_sync_snapshot: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_or_create_resource_id(&self, path: &str) -> u32 {
        self.stream_state_map
            .get(path)
            .and_then(|s| s.as_option())
            .unwrap_or_else(|| self.next_resource_id.fetch_add(1, Ordering::SeqCst))
    }
}

impl HostState for ServerState {
    type NetworkHandler = ServerNetworkHandler;
    const ROLE: HostRole = HostRole::Server;

    fn apply_extra_plugin_fields(&self) -> anyhow::Result<()> {
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
