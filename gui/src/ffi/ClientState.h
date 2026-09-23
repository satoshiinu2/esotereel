#pragma once

#include "Result.h"
#include "esotereel_gui_helper.h"
#include "ffi/project/Project.h"
#include <QWidget>
#include <qcontainerfwd.h>
#include <qguiapplication_platform.h>
#include <qlogging.h>
#include <qwindowdefs.h>
#include <sys/types.h>

namespace esotereel_gui_helper {
struct ClientNetworkHandler;
}

namespace esotereel {
class Requests;

using RawClientStateHandle = esotereel_gui_helper::ClientStateHandle;
using GuiCallbacks = esotereel_gui_helper::GuiCallbacks;

class ClientState {

  private:
    const RawClientStateHandle *network_ptr;
    bool isWayland;

  public:
    ClientState(GuiCallbacks callbacks, QString stdPluginDir, QString workingDir);
    ~ClientState();
    ClientState(const ClientState &) = delete;
    ClientState &operator=(const ClientState &) = delete;

    ClientState(ClientState &&other) noexcept;
    ClientState &operator=(ClientState &&other) noexcept;

    operator const RawClientStateHandle *() const noexcept {
        return network_ptr;
    }

    bool isValid() const {
        return network_ptr != nullptr;
    }

    bool run(QString addr);
    Result<Project> getProject() const;
    Result<void> bootstrap() const;
    Result<void> logDirectoriesInfo() const;

    Requests requests() const;
};
} // namespace esotereel