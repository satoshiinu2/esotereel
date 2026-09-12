use std::sync::{Arc, Mutex, OnceLock, RwLock};

use dashmap::DashMap;

use crate::decode::{streamplayer::StreamPlayer, videostreamer::VideoStreamer};
use crate::dirs::Directories;
use crate::plugin::{PluginLoadResult, PluginLoader};
use crate::project::Project;

pub mod decode;
pub mod dirs;
pub mod plugin;
pub mod project;
pub mod render;
pub mod requests;
pub mod responces;
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
    pub fn new(
        dirs_def: Directories,
        shared_plugin_loader: Option<Arc<Mutex<PluginLoader>>>,
    ) -> Self {
        Self {
            project: None,
            dir: dirs_def,
            path_to_stream: DashMap::new(),
            plugin_loader: shared_plugin_loader
                .unwrap_or(Arc::new(Mutex::new(PluginLoader::new()))),
        }
    }

    pub async fn load_plugins(&mut self, role: HostRole) -> anyhow::Result<Vec<PluginLoadResult>> {
        let mut loader = self.plugin_loader.lock().expect("mutex poisoned");
        loader.load_from_disk(&self.dir, role).await
    }
}

// スレッド間で移動させること自体は問題ない
// ただし複数スレッドから書き込まない
unsafe impl Send for VideoStreamer {}
unsafe impl Sync for VideoStreamer {}
unsafe impl Send for StreamPlayer {}
unsafe impl Sync for StreamPlayer {}
