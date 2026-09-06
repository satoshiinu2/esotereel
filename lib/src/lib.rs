use std::sync::{Arc, RwLock};
use std::sync::{OnceLock, atomic::AtomicU32};
use tokio::sync::Notify;

use std::sync::atomic::Ordering;

use crate::decode::{streamplayer::StreamPlayer, videostreamer::VideoStreamer};
use crate::plugin::PluginLoader;
use crate::project::Project;
use dashmap::DashMap;

pub mod decode;
pub mod pathes;
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

pub struct ClientState {
    pub project: Option<Arc<RwLock<Project>>>,

    pub path_to_stream: DashMap<String, StreamState>,
    pub streams: DashMap<u32, StreamPlayer>,
    
    pub plugin_loader: PluginLoader,
}

impl ClientState {
    pub fn new() -> Self {
        Self {
            project: None,
            path_to_stream: DashMap::new(),
            streams: DashMap::new(),
            plugin_loader: PluginLoader::new(),
        }
    }
    
    /// プラグインを並列で読み込む
    /// このメソッドはasyncなので、tokio runtime内で呼び出す必要がある
    pub async fn load_plugins(&mut self) -> anyhow::Result<Vec<crate::plugin::PluginLoadResult>> {
        self.plugin_loader.load_from_disk(HostRole::Client).await
    }
}

pub struct ServerState {
    pub project: Option<Arc<RwLock<Project>>>,

    pub path_to_stream: DashMap<String, StreamState>,
    pub streams: DashMap<u32, VideoStreamer>,
    pub next_resource_id: AtomicU32,

    pub dirty_signal: Arc<Notify>,
    
    pub plugin_loader: PluginLoader,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            project: None,
            path_to_stream: DashMap::new(),
            streams: DashMap::new(),
            next_resource_id: AtomicU32::new(0),
            dirty_signal: Arc::new(Notify::new()),
            plugin_loader: PluginLoader::new(),
        }
    }
    
    /// プラグインを並列で読み込む
    /// このメソッドはasyncなので、tokio runtime内で呼び出す必要がある
    pub async fn load_plugins(&mut self) -> anyhow::Result<Vec<crate::plugin::PluginLoadResult>> {
        self.plugin_loader.load_from_disk(HostRole::Server).await
    }
    pub fn get_or_create_resource_id(&mut self, path: &str) -> u32 {
        self.path_to_stream
            .get(path)
            .and_then(|s| s.as_option())
            .unwrap_or_else(|| self.next_resource_id.fetch_add(1, Ordering::SeqCst))
    }
}
// スレッド間で移動させること自体は問題ない
// ただし複数スレッドから書き込まない
unsafe impl Send for VideoStreamer {}
unsafe impl Sync for VideoStreamer {}
unsafe impl Send for StreamPlayer {}
unsafe impl Sync for StreamPlayer {}
