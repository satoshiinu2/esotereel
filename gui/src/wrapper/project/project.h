#pragma once

#include "esotereel_gui_helper.h"
#include "timeline.h"

using RawProject = esotereel_gui_helper::_Project;

class Project {
    const void *guard_ptr;
    const RawProject *project_ptr;

  public:
    Project(const void *g, const RawProject *p) : guard_ptr(g), project_ptr(p) {}

    static Project byGuard(const void *guard_ptr) {
        if (!guard_ptr)
            return Project::invalid();

        const RawProject *project_ptr;

        esotereel_gui_helper::project_guard_get_project_from_guard(guard_ptr, &project_ptr);
        return Project{
            guard_ptr,
            project_ptr,
        };
    }

    static Project invalid() {
        return Project(nullptr, nullptr);
    }

    ~Project() {
        esotereel_gui_helper::client_network_handler_app_state_project_unlock_read(guard_ptr); // デストラクタでRust側のロックを解除
    }

    bool isValid() const noexcept { return project_ptr != nullptr; }

    Timeline timelineOf(size_t index) const noexcept { return Timeline(esotereel_gui_helper::project_get_timeline(project_ptr, index)); }
    size_t timelineCount() const noexcept { return esotereel_gui_helper::project_get_timeline_count(project_ptr); }
    void debugLog() const noexcept { esotereel_gui_helper::project_debug_log(project_ptr); }
};
