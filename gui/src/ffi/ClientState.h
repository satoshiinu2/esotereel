#pragma once

#include "Result.h"
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

using RawClientNetworkHandler = esotereel_gui_helper::ClientStateHandle;

class ClientState {

  private:
    const RawClientNetworkHandler *network_ptr;
    bool isWayland;

  public:
    ClientState(QString stdPluginDir, QString workingDir);
    ~ClientState();
    ClientState(const ClientState &) = delete;
    ClientState &operator=(const ClientState &) = delete;

    ClientState(ClientState &&other) noexcept;
    ClientState &operator=(ClientState &&other) noexcept;

    operator const RawClientNetworkHandler *() const noexcept {
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