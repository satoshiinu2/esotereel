#pragma once

#include "esotereel_gui_helper.h"
#include "ffi/ClientNetworkHandler.h"
#include <QString>

namespace esotereel::InternalServer {

bool start(ClientNetworkHandler &network, QString addr, void (*OnConnectedFn)(bool, esotereel_gui_helper::StringView),
           QString stdPluginDir, QString workingDir);
} // namespace esotereel::InternalServer