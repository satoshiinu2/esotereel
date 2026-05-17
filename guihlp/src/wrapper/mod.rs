use std::any::Any;

use esotereel_lib::{project::Project, util::result::format_any_error};

use crate::{ON_CONNECTED_CALLBACKS, network::OnConnectedFn};

pub mod commands;
pub mod debug_streams;
pub mod internalserver;
pub mod logger;
pub mod network;
pub mod project;
pub mod wgpuutil;
pub mod requests;
pub mod stringview;

pub type OnServerReadyFn = extern "C" fn(bool);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_on_connected_callback(callback: OnConnectedFn) {
    ON_CONNECTED_CALLBACKS.set(callback).ok();
}

pub type ProjectGuard<'a> = std::sync::RwLockReadGuard<'a, Option<Project>>;

pub(crate) fn log_if_panicked<T>(
    result: Result<T, Box<dyn Any + Send>>,
    context: &str,
) -> Option<String> {
    if let Err(panic_info) = result {
        let msg = format_any_error(panic_info);

        log::error!("FFI: Panic occurred in {}! Message: {}", context, msg);
        Some(msg)
    } else {
        None
    }
}
