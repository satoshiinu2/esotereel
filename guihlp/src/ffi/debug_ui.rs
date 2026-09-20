use std::sync::Arc;

use crate::{WrapperErrorCode, ffi::state::ClientStateHandle};

#[unsafe(no_mangle)]
pub extern "C" fn launch_debug_window(ptr_state: *const ClientStateHandle) -> WrapperErrorCode {
    let state = ClientStateHandle::from_ptr(ptr_state);

    let state_guard = state.lock().expect("mutex poisoned");

    let stream_state_map = Arc::clone(&state_guard.stream_state_map);

    drop(state_guard);

    std::thread::spawn(|| {
        if let Err(e) = esotereel_debug_ui::launch_debug_window(stream_state_map) {
            eprintln!("Failed to launch debug window: {}", e);
        }
    });
    WrapperErrorCode::ok()
}
