#pragma once

#include <QString>
#include <cstddef>
#include <cstdint>
#include <vector>

#include "ClientNetworkHandler.h"
#include "esotereel_gui_helper.h"

namespace esotereel {
using TimelineId = esotereel_gui_helper::TimelineId;
using LayerId = esotereel_gui_helper::LayerId;
using ClipId = esotereel_gui_helper::ClipId;
using Tick = esotereel_gui_helper::Tick;

class Requests {
    const esotereel_gui_helper::ClientNetworkHandler *ptr_network;

  public:
    Requests(const ClientNetworkHandler *network);
    void newProject();
    void moveClips(TimelineId timelineIdx, const std::vector<ClipId> &clipIds, Tick posMoved, Tick durationMoved,
                   int64_t layerMoved) noexcept;
    void addClipAt(TimelineId timelineIdx, Tick position, uint64_t layerId) noexcept;
    void addLayer(TimelineId timelineIdx, std::optional<uint64_t> parentLayerId, std::optional<uint32_t> insertIndex,
                  const std::string &name, bool isFolder) noexcept;
    void loadStream(QString path) noexcept;
    void fetchFrame(TimelineId timelineIdx, Tick playhead, std::pair<Tick, Tick> visible_range) noexcept;

    void debugProjectLog() noexcept;
};
} // namespace esotereel