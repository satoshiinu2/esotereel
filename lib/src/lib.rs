use std::sync::{Arc, OnceLock, RwLock, atomic::AtomicU32};

use crate::{
    decode::{streamplayer::StreamPlayer, videostreamer::VideoStreamer},
    project::Project,
};
use dashmap::DashMap;

pub mod decode;
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
pub struct ClientState {
    pub project: RwLock<Option<Arc<Project>>>,

    pub path_to_stream: Arc<DashMap<String, StreamState>>,
    pub streams: Arc<DashMap<u32, StreamPlayer>>,
}

impl ClientState {
    pub fn new() -> Self {
        Self {
            project: RwLock::new(None),
            path_to_stream: Arc::new(DashMap::new()),
            streams: Arc::new(DashMap::new()),
        }
    }
}

pub struct ServerState {
    pub project: Arc<RwLock<Option<Project>>>,

    pub path_to_stream: Arc<DashMap<String, StreamState>>,
    pub streams: Arc<DashMap<u32, VideoStreamer>>,
    pub next_resource_id: Arc<AtomicU32>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            project: Arc::new(RwLock::new(None)),
            path_to_stream: Arc::new(DashMap::new()),
            streams: Arc::new(DashMap::new()),
            next_resource_id: Arc::new(AtomicU32::new(0)),
        }
    }
}
// スレッド間で移動させること自体は問題ない
// ただし複数スレッドから書き込まない
unsafe impl Send for VideoStreamer {}
unsafe impl Sync for VideoStreamer {}
unsafe impl Send for StreamPlayer {}
unsafe impl Sync for StreamPlayer {}
