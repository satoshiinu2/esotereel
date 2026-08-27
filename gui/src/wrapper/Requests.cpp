#include "Requests.h"
#include "ClientNetworkHandler.h"
#include "StringView.h"
#include "esotereel_gui_helper.h"
#include "wrapper/project/RenderRows.h"
#include <cstdint>

namespace esotereel {
Requests::Requests(const ClientNetworkHandler *network) : ptr_network(*network) {}

void Requests::newProject() {
    esotereel_gui_helper::req_new_project(ptr_network);
}

void Requests::moveClips(TimelineId timelineId, const std::vector<ClipId> &clipIds, TimelineTick posMoved,
                         TimelineTick durationMoved, int64_t layerMoved) noexcept {
    esotereel_gui_helper::req_cmd_clip_move_mul(ptr_network, timelineId, clipIds.data(), clipIds.size(), posMoved,
                                                durationMoved, layerMoved);
}

void Requests::addClipAt(TimelineId timelineId, TimelineTick position, LayerId layerId) noexcept {
    esotereel_gui_helper::req_cmd_add_clip_dummy(ptr_network, timelineId, position, layerId);
}

void Requests::addLayer(TimelineId timelineId, std::optional<uint64_t> parentLayerId,
                        std::optional<uint32_t> insertIndex, const std::string &name, bool isFolder) noexcept {
    esotereel_gui_helper::req_cmd_add_layer(ptr_network, timelineId, parentLayerId.has_value(),
                                            parentLayerId.value_or(0), insertIndex.has_value(), insertIndex.value_or(0),
                                            StringView::fromStdString(name), isFolder);
}

void Requests::loadStream(QString path) noexcept {
    QByteArray pathUtf8 = path.toUtf8();
    auto pathView = StringView::fromQUtf8String(pathUtf8);

    esotereel_gui_helper::req_load_stream(ptr_network, pathView);
}

void Requests::fetchFrame(TimelineId timelineId, TimelineTick playhead,
                          std::pair<TimelineTick, TimelineTick> visible_range) noexcept {
    esotereel_gui_helper::req_fetch_frame(ptr_network, timelineId, playhead, visible_range.first, visible_range.second);
}

void Requests::debugProjectLog() noexcept {
    esotereel_gui_helper::req_project_log(ptr_network);
}
} // namespace esotereel