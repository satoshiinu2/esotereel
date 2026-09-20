#include "Requests.h"
#include "ClientState.h"
#include "StringView.h"
#include "esotereel_gui_helper.h"
#include "ffi/project/RenderRows.h"

namespace esotereel {

Requests::Requests(const ClientState *network) : ptr_network(*network) {}

void Requests::newProject() {
    esotereel_gui_helper::req_new_project(ptr_network);
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