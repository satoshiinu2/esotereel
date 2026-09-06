use std::sync::{Arc, Mutex, RwLock};
use std::sync::{OnceLock, atomic::AtomicU32};
use tokio::sync::Notify;

use std::sync::atomic::Ordering;

use crate::decode::{streamplayer::StreamPlayer, videostreamer::VideoStreamer};
use crate::dirs::Directories;
use crate::plugin::PluginLoader;
use crate::project::Project;
use dashmap::DashMap;

pub mod decode;
pub mod dirs;
pub mod plugin;
pub mod project;
pub mod render;
pub mod requests;
pub mod responces;
pub mod setting;
pub mod util;

pub type OnSendFn = extern "C" fn(u32, *const u8, usize);
pub const CLIENT_ALL: u32 = u32::MAX;
pub const NO_CLIENT: u32 = u32::MAX;

pub(crate) static SEND_REQUEST_CALLBACK: OnceLock<OnSendFn> = OnceLock::new();
pub(crate) static SEND_RESPONSE_CALLBACK: OnceLock<OnSendFn> = OnceLock::new();

pub fn set_send_request_callback(callback: OnSendFn) {
    SEND_REQUEST_CALLBACK.set(callback).ok();
}

pub fn set_send_response_callback(callback: OnSendFn) {
    SEND_RESPONSE_CALLBACK.set(callback).ok();
}

#[derive(Clone, Copy, Debug)]
pub enum StreamState {
    Loading,
    Loaded(u32),
}

impl StreamState {
    pub fn as_option(&self) -> Option<u32> {
        if let StreamState::Loaded(id) = self {
            Some(*id)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum HostRole {
    Client,
    Server,
}

pub struct CommonState {
    pub project: Option<Arc<RwLock<Project>>>,

    pub dir: Directories,

    pub path_to_stream: DashMap<String, StreamState>,

    pub plugin_loader: Arc<Mutex<PluginLoader>>,
}

impl CommonState {
    pub fn new(dirs_def: Directories, plugin_loader: Option<Arc<Mutex<PluginLoader>>>) -> Self {
        Self {
            project: None,
            dir: dirs_def,
            path_to_stream: DashMap::new(),
            plugin_loader: plugin_loader.unwrap_or(Arc::new(Mutex::new(PluginLoader::new()))),
        }
    }

    pub async fn load_plugins(
        &mut self,
        role: HostRole,
    ) -> anyhow::Result<Vec<crate::plugin::PluginLoadResult>> {
        self.plugin_loader
            .lock()
            .expect("mutex poisoned")
            .load_from_disk(&self.dir, role)
            .await
    }
}

pub struct ClientState {
    pub common: CommonState,

    pub streams: DashMap<u32, StreamPlayer>,
}

impl ClientState {
    pub fn new(dirs_def: Directories) -> Self {
        Self {
            common: CommonState::new(dirs_def, None),
            streams: DashMap::new(),
        }
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

pub struct ServerState {
    pub common: CommonState,

    pub streams: DashMap<u32, VideoStreamer>,
    pub next_resource_id: AtomicU32,

    pub dirty_signal: Arc<Notify>,
}

impl ServerState {
    pub fn new(dirs_def: Directories, plugin_loader: Option<Arc<Mutex<PluginLoader>>>) -> Self {
        Self {
            common: CommonState::new(dirs_def, plugin_loader),
            streams: DashMap::new(),
            next_resource_id: AtomicU32::new(0),
            dirty_signal: Arc::new(Notify::new()),
        }
    }

    pub fn get_or_create_resource_id(&mut self, path: &str) -> u32 {
        self.path_to_stream
            .get(path)
            .and_then(|s| s.as_option())
            .unwrap_or_else(|| self.next_resource_id.fetch_add(1, Ordering::SeqCst))
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
// スレッド間で移動させること自体は問題ない
// ただし複数スレッドから書き込まない
unsafe impl Send for VideoStreamer {}
unsafe impl Sync for VideoStreamer {}
unsafe impl Send for StreamPlayer {}
unsafe impl Sync for StreamPlayer {}
