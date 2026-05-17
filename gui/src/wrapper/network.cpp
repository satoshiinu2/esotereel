#include "network.h"
#include "esotereel_gui_helper.h"
#include "project/project.h"
#include "requests.h"

using WrapperErrorCode = esotereel_gui_helper::WrapperErrorCode;

ClientNetworkHandler::ClientNetworkHandler() {
    auto res = esotereel_gui_helper::client_network_handler_new(&network_ptr);
    if (res != WrapperErrorCode::Ok) {
        qCritical() << "Failed to create ClientNetworkHandler:" << (int)res;
        network_ptr = nullptr;
    }
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

    auto res = esotereel_gui_helper::client_network_handler_run(network_ptr, addrView);
    if (res != esotereel_gui_helper::WrapperErrorCode::Ok) {
        qWarning() << "Failed to start network worker:" << (int)res;
    }
    return res == esotereel_gui_helper::WrapperErrorCode::Ok;
}

Project ClientNetworkHandler::getProject() const {
    if (!isValid()) {
        return Project::invalid();
    }

    const void *guard_ptr;
    esotereel_gui_helper::client_network_handler_app_state_project_lock_read(network_ptr, &guard_ptr);
    return Project::byGuard(guard_ptr);
}

Requests ClientNetworkHandler::requests() const {
    return Requests(this);
}