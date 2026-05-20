use esotereel_lib::{project::commands::Command, requests::Request, util::slot_map::SlotMapKey};

use crate::{WrapperResult, network::ClientNetworkHandler, wrapper::stringview::StringView};

#[unsafe(no_mangle)]
pub extern "C" fn req_test(ptr_network: *const ClientNetworkHandler) {
    let network = unsafe { &*ptr_network };

    let req = Request::Test;
    network.send(&req);
}

#[unsafe(no_mangle)]
pub extern "C" fn req_new_project(ptr_network: *const ClientNetworkHandler) {
    let network = unsafe { &*ptr_network };

    let req = Request::NewProject;
    network.send(&req);
}

impl ClientNetworkHandler {
    pub(super) fn req_command(&self, timeline_map_key: SlotMapKey, command: Command) {
        let req = Request::Command {
            command,
            timeline_map_key,
        };

        self.send(&req);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn req_load_stream(
    ptr_network: *const ClientNetworkHandler,
    path: StringView,
) -> WrapperResult {
    let network = unsafe { &*ptr_network };

    let Some(path) = path.as_str() else {
        return WrapperResult::invalid_string_error();
    };
    let path = path.to_string();

    let req = Request::InitStream { path };
    network.send(&req);
    WrapperResult::ok()
}
