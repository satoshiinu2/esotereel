#pragma once

#include "Result.h"
#include "wrapper/project/Project.h"
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

using RawClientNetworkHandler = esotereel_gui_helper::ClientNetworkHandler;

class ClientNetworkHandler {

  private:
    const RawClientNetworkHandler *network_ptr;
    bool isWayland;

  public:
    ClientNetworkHandler();
    ~ClientNetworkHandler();
    ClientNetworkHandler(const ClientNetworkHandler &) = delete;
    ClientNetworkHandler &operator=(const ClientNetworkHandler &) = delete;

    ClientNetworkHandler(ClientNetworkHandler &&other) noexcept;
    ClientNetworkHandler &operator=(ClientNetworkHandler &&other) noexcept;

    operator const RawClientNetworkHandler *() const noexcept {
        return network_ptr;
    }

    bool isValid() const {
        return network_ptr != nullptr;
    }

    bool run(QString addr);
    Result<Project> getProject() const;

    Requests requests() const;
};
} // namespace esotereel