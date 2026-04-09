#pragma once

#include "esotereel_gui_helper.h"
#include "timeline.h"

using RawProject = esotereel_gui_helper::Project;

class MProject {
    const RawProject *raw_ptr;

  public:
    MProject(const RawProject *p) noexcept : raw_ptr(p) {}
    bool isValid() const noexcept { return raw_ptr != nullptr; }

    MTimeline timelineOf(size_t index) const noexcept { return MTimeline(esotereel_gui_helper::project_get_timeline(raw_ptr, index)); }
    size_t timelineCount() const noexcept { return esotereel_gui_helper::project_get_timeline_count(raw_ptr); }
};

inline MProject getProject() noexcept {
    return MProject(esotereel_gui_helper::get_project());
}