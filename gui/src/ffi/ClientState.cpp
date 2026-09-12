#include "ClientState.h"
#include "Requests.h"
#include "Result.h"
#include "StringView.h"
#include "esotereel_gui_helper.h"
#include "ffi/WrapperResult.h"
#include "project/Project.h"

namespace esotereel {
ClientState::ClientState(QString stdPluginDir, QString workingDir) {

    QByteArray stdPluginDirUtf8 = stdPluginDir.toUtf8();
    auto stdPluginDirView = StringView::fromQUtf8String(stdPluginDirUtf8);

    QByteArray workingDirUtf8 = workingDir.toUtf8();
    auto workingDirView = StringView::fromQUtf8String(workingDirUtf8);

    auto result = esotereel_gui_helper::client_state_new(&network_ptr, stdPluginDirView, workingDirView);
    checkWrapperResult(result);
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
    return checkWrapperResult(result);
}

Result<Project> ClientState::getProject() const {
    if (!isValid()) {
        return Result<Project>::error("Invalid network handler");
    }

    const void *guard_ptr;
    auto result = esotereel_gui_helper::client_state_project_lock_read(network_ptr, &guard_ptr);

    if (result != WrapperErrorCode::Ok) {
        return wrapperResultToResult<Project>(result, Project::invalid());
    }

    return Project::byGuard(guard_ptr);
}

Result<void> ClientState::bootstrap() const {
    if (!isValid()) {
        return Result<void>::error("Invalid network handler");
    }

    auto result = esotereel_gui_helper::client_state_bootstrap(network_ptr);

    if (result != WrapperErrorCode::Ok) {
        return wrapperResultToResultVoid(result);
    }

    return {};
}

Result<void> ClientState::logDirectoriesInfo() const {
    if (!isValid()) {
        return Result<void>::error("Invalid network handler");
    }

    auto result = esotereel_gui_helper::client_state_log_directories_info(network_ptr);

    if (result != WrapperErrorCode::Ok) {
        return wrapperResultToResultVoid(result);
    }

    return {};
}

Requests ClientState::requests() const {
    return Requests(this);
}
} // namespace esotereel