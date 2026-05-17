#pragma once

#include <cmath>
#include <cstddef>

namespace esotereel_gui_helper {
struct Project;
}
using RawProject = esotereel_gui_helper::Project;

class Timeline;

class Project {
    const void *guard_ptr;
    const RawProject *project_ptr;

  public:
    Project(const void *g, const RawProject *p);
    ~Project();

    static Project byGuard(const void *guard_ptr);
    static Project invalid();

    bool isValid() const noexcept;
    Timeline timelineOf(size_t index) const noexcept;
    size_t timelineCount() const noexcept;
    void debugLog() const noexcept;
};
