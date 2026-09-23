#pragma once

#include <QString>
#include <cstdint>
#include <vector>

#include "ClientState.h"
#include "esotereel_gui_helper.h"

namespace esotereel {
using RawClientStateHandle = esotereel_gui_helper::ClientStateHandle;
using RawCommandQueue = esotereel_gui_helper::CommandQueue;
using TimelineId = esotereel_gui_helper::TimelineId;
using LayerId = esotereel_gui_helper::LayerId;
using LayerFolderId = esotereel_gui_helper::LayerFolderId;
using ClipId = esotereel_gui_helper::ClipId;
using TimelineTick = esotereel_gui_helper::TimelineTick;
using OptionProject = esotereel_gui_helper::OptionProject;

class CommandQueue {
    const RawClientStateHandle *ptr_state;
    RawCommandQueue *ptr_queue;

  public:
    explicit CommandQueue(const ClientState *network);

    ~CommandQueue() {
        Result<void>(esotereel_gui_helper::command_queue_drop(ptr_queue)).unwrap();
    }

    operator RawCommandQueue *() const noexcept {
        return ptr_queue;
    }

    void sendAll();

    void moveClips(const Project &project, TimelineId timelineIdx, const std::vector<ClipId> &clipIds,
                   TimelineTick posMoved, TimelineTick durationMoved, int64_t layerMoved) noexcept;

    Result<void> addClipAt(TimelineId timelineIdx, TimelineTick position, uint64_t layerId) noexcept;
    void addLayer(TimelineId timelineIdx, std::optional<uint64_t> parentLayerId, std::optional<uint32_t> insertIndex,
                  const std::string &name) noexcept;
    void addFolder(TimelineId timelineIdx, std::optional<uint64_t> parentLayerId, std::optional<uint32_t> insertIndex,
                   const std::string &name) noexcept;
};
} // namespace esotereel