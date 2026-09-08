#pragma once

#include <QString>
#include <cstdint>
#include <vector>

#include "ClientNetworkHandler.h"
#include "esotereel_gui_helper.h"

namespace esotereel {
using TimelineId = esotereel_gui_helper::TimelineId;
using LayerId = esotereel_gui_helper::LayerId;
using LayerFolderId = esotereel_gui_helper::LayerFolderId;
using ClipId = esotereel_gui_helper::ClipId;
using TimelineTick = esotereel_gui_helper::TimelineTick;

class Requests {
    const esotereel_gui_helper::ClientNetworkHandler *ptr_network;

  public:
    Requests(const ClientNetworkHandler *network);
    void newProject();
    void moveClips(TimelineId timelineIdx, const std::vector<ClipId> &clipIds, TimelineTick posMoved,
                   TimelineTick durationMoved, int64_t layerMoved) noexcept;

    void addClipAt(TimelineId timelineIdx, TimelineTick position, uint64_t layerId) noexcept;
    void addLayer(TimelineId timelineIdx, std::optional<uint64_t> parentLayerId, std::optional<uint32_t> insertIndex,
                  const std::string &name) noexcept;
    void addFolder(TimelineId timelineIdx, std::optional<uint64_t> parentLayerId, std::optional<uint32_t> insertIndex,
                   const std::string &name) noexcept;

    void loadStream(QString path) noexcept;
    void fetchFrame(TimelineId timelineIdx, TimelineTick playhead,
                    std::pair<TimelineTick, TimelineTick> visible_range) noexcept;

    void debugProjectLog() noexcept;
};
} // namespace esotereel