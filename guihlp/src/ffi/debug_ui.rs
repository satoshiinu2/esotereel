use std::sync::Arc;

use esotereel_lib::util::result::{EsotereelError, MutexExt};

use crate::ffi::{result::FfiResultVoid, state::ClientStateHandle};

#[unsafe(no_mangle)]
pub extern "C" fn launch_debug_window(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    if ptr_state.is_null() {
        return FfiResultVoid::err(EsotereelError::NullPointer("ptr_state".to_owned()).into());
    }

    let state = ClientStateHandle::from_ptr(ptr_state);
    let stream_state_map = state.stream_state_map_arc();

    std::thread::spawn(move || {
        if let Err(e) = esotereel_debug_ui::launch_debug_window(stream_state_map) {
            eprintln!("Failed to launch debug window: {}", e);
        }
    });
    FfiResultVoid::ok()
}
