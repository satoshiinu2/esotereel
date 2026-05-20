#include "requests.h"
#include "esotereel_gui_helper.h"
#include "network.h"
#include "stringview.h"
#include <cstdint>

Requests::Requests(const ClientNetworkHandler *network) : raw_ptr(*network) {}

void Requests::newProject() {
    esotereel_gui_helper::req_new_project(raw_ptr);
}

void Requests::moveClips(uint64_t timelineIdx, const std::vector<uint64_t> &clipIds, int64_t posMoved,
                         int64_t durationMoved, int64_t layerMoved) noexcept {
    esotereel_gui_helper::req_cmd_clip_move_mul(raw_ptr, timelineIdx, clipIds.data(), clipIds.size(), posMoved,
                                                durationMoved, layerMoved);
}

void Requests::addClipAt(uint64_t timelineIdx, int64_t position, uint64_t layerOrder) noexcept {
    esotereel_gui_helper::req_cmd_add_clip_dummy(raw_ptr, timelineIdx, position, layerOrder);
}

void Requests::loadStream(QString path) noexcept {
    QByteArray pathUtf8 = path.toUtf8();
    auto pathView = StringView::fromQUtf8String(pathUtf8);

    esotereel_gui_helper::req_load_stream(raw_ptr, pathView);
}