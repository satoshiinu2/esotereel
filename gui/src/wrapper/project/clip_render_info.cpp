#include "clip_render_info.h"
#include "esotereel_gui_helper.h"
#include "project.h"
#include "timeline.h"
#include <cstdint>
#include <span>
#include <vector>

using FfiLayerRow = esotereel_gui_helper::FfiLayerRow;
using ClipRenderInfo = esotereel_gui_helper::ClipRenderInfo;
using RenderRowsResult = esotereel_gui_helper::RenderRowsResult;
using RawProject = esotereel_gui_helper::Project;
using RawTimeline = esotereel_gui_helper::Timeline;

RenderRows::RenderRows(const Project &project, const Timeline &timeline, const std::vector<uint64_t> &openIds) {
    auto project_ptr = static_cast<const RawProject *>(project);
    auto timeline_ptr = static_cast<const RawTimeline *>(timeline);

    render_rows_build(project_ptr, timeline_ptr, openIds.data(), openIds.size(), &ptr);
    render_rows_get_rows(ptr, &rowsPtr, &rowsLen);
    render_rows_get_clips(ptr, &clipsPtr, &clipsLen);
}

RenderRows::~RenderRows() {
    render_rows_free(ptr);
}

std::span<const FfiLayerRow> RenderRows::rows() const {
    return {rowsPtr, rowsLen};
}

std::span<const ClipRenderInfo> RenderRows::clipsFor(const FfiLayerRow &row) const {
    return {clipsPtr + row.clip_start, row.clip_count};
}