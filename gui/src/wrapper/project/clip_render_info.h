#pragma once

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
using TimelineId = esotereel_gui_helper::TimelineId;

class RenderRows {
  private:
    RenderRowsResult *ptr = nullptr;
    const FfiLayerRow *rowsPtr = nullptr;
    size_t rowsLen = 0;
    const ClipRenderInfo *clipsPtr = nullptr;
    size_t clipsLen = 0;

  public:
    RenderRows(const Project &project, const TimelineId &timeline_id, const std::vector<uint64_t> &openIds,
               const std::vector<uint64_t> &openFolderIds);
    ~RenderRows();

    // コピー禁止（二重解放を防止）
    RenderRows(const RenderRows &) = delete;
    RenderRows &operator=(const RenderRows &) = delete;

    // ムーブ対応
    RenderRows(RenderRows &&other) noexcept;
    RenderRows &operator=(RenderRows &&other) noexcept;

    std::span<const FfiLayerRow> rows() const;
    std::span<const ClipRenderInfo> clipsFor(const FfiLayerRow &row) const;
    bool isValid() const noexcept {
        return ptr != nullptr;
    }
};