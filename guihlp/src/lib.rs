use crate::responces::on_responce_recveve;
pub use esotereel_lib::decode::streamplayer::StreamPlayer;
pub use esotereel_lib::project::Layer;
pub use esotereel_lib::project::LayerFolder;
pub use esotereel_lib::project::Project;
pub use esotereel_lib::project::Timeline;
pub use esotereel_lib::project::clip::Clip;
pub use esotereel_lib::project::ids::{
    ClipId, LayerDelta, LayerFolderId, LayerId, ScriptId, TimelineId,
};

pub mod ffi;
mod network;
pub mod responces;
pub mod state;

pub mod commands;
pub mod logger;
pub mod render;
pub mod settings;

#[repr(C)]
pub struct GuiCallbacks {
    pub on_test: extern "C" fn(),
    pub mark_dirty_timeline: extern "C" fn(timeline_type: TimelineId),
    pub on_connected: extern "C" fn(),
}

/// # Safety
///
/// If `len > 0` and `ptr` is non-null, `ptr` must point to a valid
/// contiguous array of at least `len` initialized `T`s, and the memory
/// must remain valid for the returned slice's lifetime.
pub unsafe fn slice_from_ptr_or_empty<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}
