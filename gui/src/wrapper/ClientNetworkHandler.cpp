#include "ClientNetworkHandler.h"
#include "Requests.h"
#include "Result.h"
#include "StringView.h"
#include "esotereel_gui_helper.h"
#include "project/Project.h"
#include "wrapper/WrapperResult.h"

namespace esotereel {
ClientNetworkHandler::ClientNetworkHandler() {
    auto result = esotereel_gui_helper::client_network_handler_new(&network_ptr);
    checkWrapperResult(result);
}

ClientNetworkHandler::~ClientNetworkHandler() {
    if (network_ptr) {
        esotereel_gui_helper::client_network_handler_drop(network_ptr);
        network_ptr = nullptr;
    }
}
ClientNetworkHandler::ClientNetworkHandler(ClientNetworkHandler &&other) noexcept : network_ptr(other.network_ptr) {
    other.network_ptr = nullptr;
}

ClientNetworkHandler &ClientNetworkHandler::operator=(ClientNetworkHandler &&other) noexcept {
    if (this != &other) {
        if (network_ptr) {
            esotereel_gui_helper::client_network_handler_drop(network_ptr);
        }
        network_ptr = other.network_ptr;
        other.network_ptr = nullptr;
    }
    return *this;
}

bool ClientNetworkHandler::run(QString addr) {
    if (!isValid())
        return false;

    QByteArray addrUtf8 = addr.toUtf8();
    auto addrView = StringView::fromQUtf8String(addrUtf8);

    auto result = esotereel_gui_helper::client_network_handler_run(network_ptr, addrView);
    return checkWrapperResult(result);
}

Result<Project> ClientNetworkHandler::getProject() const {
    if (!isValid()) {
        return Result<Project>::error("Invalid network handler");
    }

    const void *guard_ptr;
    auto result = esotereel_gui_helper::client_network_handler_app_state_project_lock_read(network_ptr, &guard_ptr);

    if (result != WrapperErrorCode::Ok) {
        return wrapperResultToResult<Project>(result, Project::invalid());
    }

    return Project::byGuard(guard_ptr);
}

Requests ClientNetworkHandler::requests() const {
    return Requests(this);
}
} // namespace esotereel