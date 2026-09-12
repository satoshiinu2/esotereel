use crate::ffi::state::ClientStateHandle;

#[unsafe(no_mangle)]
pub extern "C" fn debug_streams_get_resources_arr_size(
    ptr_state: *const ClientStateHandle,
) -> usize {
    if ptr_state.is_null() {
        return 0;
    }

    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");

    state.stream_players.len()
}

#[unsafe(no_mangle)]
pub extern "C" fn debug_streams_write_resources_arr(
    ptr_state: *const ClientStateHandle,
    ptr_out_arr: *mut u32,
    safety_size: usize,
) -> bool {
    if ptr_state.is_null() {
        return false;
    }

    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");

    if ptr_out_arr.is_null() && safety_size != 0 {
        return false;
    }
    if safety_size < state.stream_players.len() {
        return false;
    }

    for (i, key) in state.stream_players.iter().map(|e| *e.key()).enumerate() {
        unsafe {
            *ptr_out_arr.add(i) = key;
        }
    }

    return true;
}

#[unsafe(no_mangle)]
pub extern "C" fn debug_streams_get_loaded_streams_sec_arr_size(
    ptr_state: *const ClientStateHandle,
    resource_id: u32,
) -> usize {
    if ptr_state.is_null() {
        return 0;
    }

    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");

    let Some(stream) = state.stream_players.get(&resource_id) else {
        return 0;
    };

    stream.frames.len()
}

#[unsafe(no_mangle)]
pub extern "C" fn debug_streams_write_loaded_streams_sec_arr(
    ptr_state: *const ClientStateHandle,
    resource_id: u32,
    ptr_out_arr: *mut f64,
    safety_size: usize,
) -> bool {
    if ptr_state.is_null() {
        return false;
    }

    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");

    let Some(stream) = state.stream_players.get(&resource_id) else {
        return false;
    };

    if ptr_out_arr.is_null() && safety_size != 0 {
        return false;
    }
    if safety_size < stream.frames.len() {
        return false;
    }

    for (i, key) in stream.frames.keys().enumerate() {
        unsafe {
            *ptr_out_arr.add(i) = key.0;
        }
    }

    return true;
}
