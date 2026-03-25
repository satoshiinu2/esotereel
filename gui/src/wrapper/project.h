#pragma once

#include "muscedit_lib.h"
#include "timeline.h"

using RawProject = muscedit_lib::Project;

class MProject {
    const RawProject *raw_ptr;

  public:
    MProject(const RawProject *p) : raw_ptr(p) {}
    bool isValid() const { return raw_ptr != nullptr; }

    MTimeline timelineOf(size_t idx) const { return MTimeline(muscedit_lib::project_get_timeline(raw_ptr, idx)); }
};

inline MProject getProject() {
    return MProject(muscedit_lib::get_project());
}