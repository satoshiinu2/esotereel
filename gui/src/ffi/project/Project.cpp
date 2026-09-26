#include "Project.h"
#include "Timeline.h"
#include "esotereel_gui_helper.h"
#include "ffi/ClientState.h"
#include "ffi/Result.h"
#include "ffi/StringView.h"

namespace esotereel {
Project::Project(const void *g, const RawOptionProject *p) : guard_ptr(g), project_ptr(p) {}

Project::~Project() {
    // RAII: Ensure lock is released even if exception occurs
    if (guard_ptr) {
        esotereel_gui_helper::client_state_project_unlock_read(guard_ptr);
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
            esotereel_gui_helper::client_state_project_unlock_read(guard_ptr);
        }
        // リソースの所有権を移動
        guard_ptr = other.guard_ptr;
        project_ptr = other.project_ptr;

        other.guard_ptr = nullptr;
        other.project_ptr = nullptr;
    }
    return *this;
}

Result<Project> Project::lockRead(const ClientState *state) {
    if (!state || !state->isValid())
        return Result<Project>::err("Invalid state");

    auto result = esotereel_gui_helper::client_state_project_lock_read(*state);

    if (!result.is_ok) {
        return Result<Project>::err(OwnedString::intoStdString(result.value.err));
    }

    const void *guard_ptr = result.value.ok;
    if (!guard_ptr) {
        return Result<Project>::err("Guard pointer is null");
    }

    return Project::byGuard(guard_ptr);
}

Result<Project> Project::byGuard(const void *guard_ptr) {
    if (!guard_ptr)
        return Result<Project>::err("Guard pointer is null");

    auto result = esotereel_gui_helper::project_guard_get_project_from_guard(guard_ptr);

    if (!result.is_ok) {
        // Clean up guard on error
        esotereel_gui_helper::client_state_project_unlock_read(guard_ptr);
        return Result<Project>::err(OwnedString::intoStdString(result.value.err));
    }

    const RawOptionProject *project_ptr = result.value.ok;
    if (!project_ptr) {
        // Clean up guard on error
        esotereel_gui_helper::client_state_project_unlock_read(guard_ptr);
        return Result<Project>::err("Project pointer is null");
    }

    return Result<Project>::ok(Project{guard_ptr, project_ptr});
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

} // namespace esotereel