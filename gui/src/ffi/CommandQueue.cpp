#include "CommandQueue.h"
#include "ClientState.h"
#include "StringView.h"
#include "esotereel_gui_helper.h"
#include "ffi/project/RenderRows.h"
#include <cstdint>

namespace esotereel {

CommandQueue::CommandQueue(const ClientState *state) : ptr_state(*state) {
    ptr_queue = esotereel_gui_helper::command_queue_new();
}

void CommandQueue::sendAll() {
    esotereel_gui_helper::command_queue_send_all(ptr_queue, ptr_state);
}

void CommandQueue::moveClips(const Project &project, TimelineId timelineId, const std::vector<ClipId> &clipIds,
                             TimelineTick posMoved, TimelineTick durationMoved, int64_t layerMoved) noexcept {
    esotereel_gui_helper::req_cmd_clip_move_mul(ptr_queue, project, timelineId, clipIds.data(), clipIds.size(),
                                                posMoved, durationMoved, layerMoved);
}

void CommandQueue::addClipAt(TimelineId timelineId, TimelineTick position, LayerId layerId) noexcept {
    esotereel_gui_helper::req_cmd_add_clip_dummy(ptr_queue, ptr_state, timelineId, position, layerId);
}

void CommandQueue::addLayer(TimelineId timelineId, std::optional<uint64_t> parentLayerId,
                            std::optional<uint32_t> insertIndex, const std::string &name) noexcept {
    esotereel_gui_helper::req_cmd_add_layer(ptr_queue, timelineId, parentLayerId.has_value(), parentLayerId.value_or(0),
                                            insertIndex.has_value(), insertIndex.value_or(0),
                                            StringView::fromStdString(name));
}
void CommandQueue::addFolder(TimelineId timelineId, std::optional<uint64_t> parentLayerId,
                             std::optional<uint32_t> insertIndex, const std::string &name) noexcept {
    esotereel_gui_helper::req_cmd_add_folder(ptr_queue, timelineId, parentLayerId.has_value(),
                                             parentLayerId.value_or(0), insertIndex.has_value(),
                                             insertIndex.value_or(0), StringView::fromStdString(name));
}
} // namespace esotereel