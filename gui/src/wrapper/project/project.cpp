#include "project.h"
#include "../exception.h"
#include "esotereel_gui_helper.h"
#include "timeline.h"

Project::Project(const void *g, const RawProject *p) : guard_ptr(g), project_ptr(p) {}

Project::~Project() {
    esotereel_gui_helper::client_network_handler_app_state_project_unlock_read(guard_ptr);
}

Project Project::byGuard(const void *guard_ptr) {
    if (!guard_ptr)
        return Project::invalid();

    const RawProject *project_ptr = nullptr;
    auto result = esotereel_gui_helper::project_guard_get_project_from_guard(guard_ptr, &project_ptr);
    if (!checkWrapperResult(result)) {
        return Project::invalid();
    }

    return Project{guard_ptr, project_ptr};
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
