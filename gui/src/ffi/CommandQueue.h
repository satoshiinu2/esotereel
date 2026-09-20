#pragma once

#include <QString>
#include <cstdint>
#include <vector>

#include "ClientState.h"
#include "esotereel_gui_helper.h"

namespace esotereel {
using TimelineId = esotereel_gui_helper::TimelineId;
using LayerId = esotereel_gui_helper::LayerId;
using LayerFolderId = esotereel_gui_helper::LayerFolderId;
using ClipId = esotereel_gui_helper::ClipId;
using TimelineTick = esotereel_gui_helper::TimelineTick;
using OptionProject = esotereel_gui_helper::OptionProject;

class CommandQueue {
    const esotereel_gui_helper::ClientStateHandle *ptr_state;
    esotereel_gui_helper::CommandQueue *ptr_queue;

  public:
    explicit CommandQueue(const ClientState *network);

    ~CommandQueue() {
        Result<void>(esotereel_gui_helper::command_queue_drop(ptr_queue)).unwrap();
    }

    void sendAll();

    void moveClips(const Project &project, TimelineId timelineIdx, const std::vector<ClipId> &clipIds,
                   TimelineTick posMoved, TimelineTick durationMoved, int64_t layerMoved) noexcept;

    void addClipAt(TimelineId timelineIdx, TimelineTick position, uint64_t layerId) noexcept;
    void addLayer(TimelineId timelineIdx, std::optional<uint64_t> parentLayerId, std::optional<uint32_t> insertIndex,
                  const std::string &name) noexcept;
    void addFolder(TimelineId timelineIdx, std::optional<uint64_t> parentLayerId, std::optional<uint32_t> insertIndex,
                   const std::string &name) noexcept;
};
} // namespace esotereel