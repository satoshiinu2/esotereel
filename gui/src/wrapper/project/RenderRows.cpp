#include "RenderRows.h"
#include "Project.h"
#include "Timeline.h"
#include "esotereel_gui_helper.h"
#include <cstdint>
#include <span>
#include <vector>

namespace esotereel {
using FfiLayerRow = esotereel_gui_helper::FfiLayerRow;
using ClipRenderInfo = esotereel_gui_helper::ClipRenderInfo;
using RenderRowsResult = esotereel_gui_helper::RenderRowsResult;
using RawProject = esotereel_gui_helper::Project;
using RawTimeline = esotereel_gui_helper::Timeline;

RenderRows::RenderRows(const Project &project, const TimelineId &timeline_id, const std::vector<uint64_t> &openIds,
                       const std::vector<uint64_t> &openFolderIds) {
    if (!project.isValid()) {
        return;
    }
    auto project_ptr = static_cast<const RawProject *>(project);
    auto timeline = project.timelineOf(timeline_id);
    if (!timeline.isValid()) {
        return;
    }

    auto err = render_rows_build(project_ptr, timeline, openIds.data(), openIds.size(), openFolderIds.data(),
                                 openFolderIds.size(), &ptr);
    if (err != esotereel_gui_helper::WrapperErrorCode::Ok || !ptr) {
        ptr = nullptr;
        return;
    }

    render_rows_get_rows(ptr, &rowsPtr, &rowsLen);
    render_rows_get_clips(ptr, &clipsPtr, &clipsLen);
}

RenderRows::~RenderRows() {
    if (ptr) {
        render_rows_free(ptr);
        ptr = nullptr;
    }
}

RenderRows::RenderRows(RenderRows &&other) noexcept
    : ptr(other.ptr), rowsPtr(other.rowsPtr), rowsLen(other.rowsLen), clipsPtr(other.clipsPtr),
      clipsLen(other.clipsLen) {
    other.ptr = nullptr;
    other.rowsPtr = nullptr;
    other.rowsLen = 0;
    other.clipsPtr = nullptr;
    other.clipsLen = 0;
}

RenderRows &RenderRows::operator=(RenderRows &&other) noexcept {
    if (this != &other) {
        if (ptr)
            render_rows_free(ptr);

        ptr = other.ptr;
        rowsPtr = other.rowsPtr;
        rowsLen = other.rowsLen;
        clipsPtr = other.clipsPtr;
        clipsLen = other.clipsLen;

        other.ptr = nullptr;
        other.rowsPtr = nullptr;
        other.rowsLen = 0;
        other.clipsPtr = nullptr;
        other.clipsLen = 0;
    }
    return *this;
}

std::span<const FfiLayerRow> RenderRows::rows() const {
    if (!rowsPtr)
        return {};
    return {rowsPtr, rowsLen};
}

std::span<const ClipRenderInfo> RenderRows::clipsFor(const FfiLayerRow &row) const {
    if (!clipsPtr || row.clip_start + row.clip_count > clipsLen) {
        return {};
    }
    return {clipsPtr + row.clip_start, row.clip_count};
}
} // namespace esotereel