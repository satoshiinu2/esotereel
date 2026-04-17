#pragma once

#include "esotereel_gui_helper.h"
#include "timeline.h"

using RawProject = esotereel_gui_helper::_Project;

class Project {
    const RawProject *raw_ptr;

  public:
    Project(const RawProject *p) noexcept : raw_ptr(p) {}
    bool isValid() const noexcept { return raw_ptr != nullptr; }

    Timeline timelineOf(size_t index) const noexcept { return Timeline(esotereel_gui_helper::project_get_timeline(raw_ptr, index)); }
    size_t timelineCount() const noexcept { return esotereel_gui_helper::project_get_timeline_count(raw_ptr); }
};

inline Project getProject() noexcept {
    return Project(esotereel_gui_helper::get_project());
}