#pragma once

#include "project.h"
#include "timeline.h"
#include <cstdint>
#include <span>
#include <vector>

namespace esotereel_gui_helper {
    struct FfiLayerRow;
    struct ClipRenderInfo;
    struct RenderRowsResult;
}
using FfiLayerRow = esotereel_gui_helper::FfiLayerRow;
using ClipRenderInfo = esotereel_gui_helper::ClipRenderInfo;
using RenderRowsResult = esotereel_gui_helper::RenderRowsResult;
using RawProject = esotereel_gui_helper::Project;
using RawTimeline = esotereel_gui_helper::Timeline;

class RenderRows {
public:
    RenderRows(const Project& project, const Timeline& timeline, const std::vector<uint64_t>& openIds) ;
    ~RenderRows() ;

    RenderRows(const RenderRows&) = delete;
    RenderRows& operator=(const RenderRows&) = delete;

    std::span<const FfiLayerRow> rows() const ;
    std::span<const ClipRenderInfo> clipsFor(const FfiLayerRow& row) const;

private:
    RenderRowsResult* ptr = nullptr;
    const FfiLayerRow* rowsPtr = nullptr;
    size_t rowsLen = 0;
    const ClipRenderInfo* clipsPtr = nullptr;
    size_t clipsLen = 0;
};