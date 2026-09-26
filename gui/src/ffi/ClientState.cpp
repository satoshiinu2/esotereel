#include "ClientState.h"
#include "CommandQueue.h"
#include "Requests.h"
#include "Result.h"
#include "StringView.h"
#include "esotereel_gui_helper.h"
#include "ffi/WrapperResult.h"
#include "project/Project.h"

namespace esotereel {
ClientState::ClientState(GuiCallbacks callbacks, QString stdPluginDir, QString workingDir) {

    QByteArray stdPluginDirUtf8 = stdPluginDir.toUtf8();
    auto stdPluginDirView = StringView::fromQUtf8String(stdPluginDirUtf8);

    QByteArray workingDirUtf8 = workingDir.toUtf8();
    auto workingDirView = StringView::fromQUtf8String(workingDirUtf8);

    auto result = esotereel_gui_helper::client_state_new(callbacks, stdPluginDirView, workingDirView);

    if (!result.is_ok) {
        std::string error = OwnedString::intoStdString(result.value.err);
        throw std::runtime_error("Failed to create client state: " + error);
    }

    network_ptr = result.value.ok;
}

ClientState::~ClientState() {
    if (network_ptr) {
        esotereel_gui_helper::client_state_drop(network_ptr);
        network_ptr = nullptr;
    }
}
ClientState::ClientState(ClientState &&other) noexcept : network_ptr(other.network_ptr) {
    other.network_ptr = nullptr;
}

ClientState &ClientState::operator=(ClientState &&other) noexcept {
    if (this != &other) {
        if (network_ptr) {
            esotereel_gui_helper::client_state_drop(network_ptr);
        }
        network_ptr = other.network_ptr;
        other.network_ptr = nullptr;
    }
    return *this;
}

bool ClientState::run(QString addr) {
    if (!isValid())
        return false;

    QByteArray addrUtf8 = addr.toUtf8();
    auto addrView = StringView::fromQUtf8String(addrUtf8);

    auto result = esotereel_gui_helper::client_state_network_run(network_ptr, addrView);

    if (!result.is_ok) {
        std::string error = OwnedString::intoStdString(result.err);
        // Log error but don't throw
        return false;
    }

    return true;
}

Result<Project> ClientState::getProject() const {
    if (!isValid()) {
        return Result<Project>::err("Invalid network handler");
    }

    auto result = esotereel_gui_helper::client_state_project_lock_read(network_ptr);

    if (!result.is_ok) {
        return Result<Project>::err(OwnedString::intoStdString(result.value.err));
    }

    return Project::byGuard(result.value.ok);
}

Result<void> ClientState::bootstrap() const {
    if (!isValid()) {
        return Result<void>::err("Invalid network handler");
    }

    auto result = esotereel_gui_helper::client_state_bootstrap(network_ptr);

    if (!result.is_ok) {
        return Result<void>::err(OwnedString::intoStdString(result.err));
    }

    return Result<void>::ok();
}

Result<void> ClientState::logDirectoriesInfo() const {
    if (!isValid()) {
        return Result<void>::err("Invalid network handler");
    }

    auto result = esotereel_gui_helper::client_state_log_directories_info(network_ptr);

    if (!result.is_ok) {
        return Result<void>::err(OwnedString::intoStdString(result.err));
    }

    return Result<void>::ok();
}

Requests ClientState::requests() const {
    return Requests(this);
}
} // namespace esotereel