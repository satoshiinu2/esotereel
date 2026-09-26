use crate::decode::{streamplayer::StreamPlayer, videostreamer::VideoStreamer};

pub mod decode;
pub mod dirs;
pub mod plugin;
pub mod project;
pub mod render;
pub mod requests;
pub mod responces;
pub mod state;
pub mod util;

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

// スレッド間で移動させること自体は問題ない
// ただし複数スレッドから書き込まない
unsafe impl Send for VideoStreamer {}
unsafe impl Sync for VideoStreamer {}
unsafe impl Send for StreamPlayer {}
unsafe impl Sync for StreamPlayer {}
