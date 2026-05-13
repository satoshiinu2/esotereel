use crate::network::ClientNetworkHandler;

#[unsafe(no_mangle)]
pub extern "C" fn debug_streams_get_resources_arr_size(
    ptr_network: *const ClientNetworkHandler,
) -> usize {
    if ptr_network.is_null() {
        return 0;
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    app_state.streams.len()
}

#[unsafe(no_mangle)]
pub extern "C" fn debug_streams_write_resources_arr(
    ptr_network: *const ClientNetworkHandler,
    ptr_out_arr: *mut u32,
    safety_size: usize,
) -> bool {
    if ptr_network.is_null() {
        return false;
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    if ptr_out_arr.is_null() && safety_size != 0 {
        return false;
    }
    if safety_size < app_state.streams.len() {
        return false;
    }

    for (i, key) in app_state.streams.iter().map(|e| *e.key()).enumerate() {
        unsafe {
            *ptr_out_arr.add(i) = key;
        }
    }

    return true;
}

#[unsafe(no_mangle)]
pub extern "C" fn debug_streams_get_loaded_streams_sec_arr_size(
    ptr_network: *const ClientNetworkHandler,
    resource_id: u32,
) -> usize {
    if ptr_network.is_null() {
        return 0;
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let Some(stream) = app_state.streams.get(&resource_id) else {
        return 0;
    };

    stream.frames.len()
}

#[unsafe(no_mangle)]
pub extern "C" fn debug_streams_write_loaded_streams_sec_arr(
    ptr_network: *const ClientNetworkHandler,
    resource_id: u32,
    ptr_out_arr: *mut f64,
    safety_size: usize,
) -> bool {
    if ptr_network.is_null() {
        return false;
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let Some(stream) = app_state.streams.get(&resource_id) else {
        return false;
    };

    if ptr_out_arr.is_null() && safety_size != 0 {
        return false;
    }
    if safety_size < stream.frames.len() {
        return false;
    }

    for (i, key) in stream.frames.iter().map(|e| e.0).enumerate() {
        unsafe {
            *ptr_out_arr.add(i) = key;
        }
    }

    return true;
}
