#include "project.h"
#include "../exception.h"
#include "../network.h"
#include "esotereel_gui_helper.h"
#include "timeline.h"

Project::Project(const void *g, const RawProject *p) : guard_ptr(g), project_ptr(p) {}

Project::~Project() {
    // RAII: Ensure lock is released even if exception occurs
    if (guard_ptr) {
        esotereel_gui_helper::client_network_handler_app_state_project_unlock_read(guard_ptr);
        guard_ptr = nullptr;
        project_ptr = nullptr;
    }
}

// 移動コンストラクタの実装
Project::Project(Project &&other) noexcept : guard_ptr(other.guard_ptr), project_ptr(other.project_ptr) {
    other.guard_ptr = nullptr;
    other.project_ptr = nullptr;
}

// 移動代入演算子の実装
Project &Project::operator=(Project &&other) noexcept {
    if (this != &other) {
        // 既存のガードを解放（例外安全のため）
        if (guard_ptr) {
            esotereel_gui_helper::client_network_handler_app_state_project_unlock_read(guard_ptr);
        }
        // リソースの所有権を移動
        guard_ptr = other.guard_ptr;
        project_ptr = other.project_ptr;

        other.guard_ptr = nullptr;
        other.project_ptr = nullptr;
    }
    return *this;
}

esotereel_gui_helper::Result<Project> Project::lockRead(const ClientNetworkHandler *network) {
    if (!network || !network->isValid())
        return esotereel_gui_helper::Result<Project>::error("Invalid network handler");

    const void *guard_ptr = nullptr;
    // C++ クラスが保持する FFI 用ポインタ (raw_ptr) を渡す
    auto result = esotereel_gui_helper::client_network_handler_app_state_project_lock_read(*network, &guard_ptr);

    if (result != WrapperErrorCode::Ok || !guard_ptr) {
        return wrapperResultToResult<Project>(result, Project::invalid());
    }

    return Project::byGuard(guard_ptr);
}

esotereel_gui_helper::Result<Project> Project::byGuard(const void *guard_ptr) {
    if (!guard_ptr)
        return esotereel_gui_helper::Result<Project>::error("Guard pointer is null");

    const RawProject *project_ptr = nullptr;
    auto result = esotereel_gui_helper::project_guard_get_project_from_guard(guard_ptr, &project_ptr);
    
    if (result != WrapperErrorCode::Ok) {
        // Clean up guard on error
        esotereel_gui_helper::client_network_handler_app_state_project_unlock_read(guard_ptr);
        return wrapperResultToResult<Project>(result, Project::invalid());
    }

    return esotereel_gui_helper::Result<Project>::ok(Project{guard_ptr, project_ptr});
}

Project Project::invalid() {
    return Project(nullptr, nullptr);
}

bool Project::isValid() const noexcept {
    return project_ptr != nullptr;
}

Timeline Project::timelineOf(size_t index) const noexcept {
    return Timeline(esotereel_gui_helper::project_get_timeline(project_ptr, index));
}

size_t Project::timelineCount() const noexcept {
    return esotereel_gui_helper::project_get_timeline_count(project_ptr);
}

void Project::debugLog() const noexcept {
    esotereel_gui_helper::project_debug_log(project_ptr);
}
