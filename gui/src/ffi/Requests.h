#pragma once

#include <QString>

#include "ClientState.h"
#include "esotereel_gui_helper.h"

namespace esotereel {
using TimelineId = esotereel_gui_helper::TimelineId;
using LayerId = esotereel_gui_helper::LayerId;
using LayerFolderId = esotereel_gui_helper::LayerFolderId;
using ClipId = esotereel_gui_helper::ClipId;
using TimelineTick = esotereel_gui_helper::TimelineTick;
using OptionProject = esotereel_gui_helper::OptionProject;

class Requests {
    const esotereel_gui_helper::ClientStateHandle *ptr_network;

  public:
    explicit Requests(const ClientState *network);
    void newProject();
    void loadStream(QString path) noexcept;
    void fetchFrame(TimelineId timelineIdx, TimelineTick playhead,
                    std::pair<TimelineTick, TimelineTick> visible_range) noexcept;

    void debugProjectLog() noexcept;
};
} // namespace esotereel