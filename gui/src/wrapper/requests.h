#pragma once

#include "esotereel_gui_helper.h"
#include <cstddef>
#include <vector>

class Requests {
  public:
    static void newProject() {
        esotereel_gui_helper::req_new_project();
    }

    static void moveClips(uint64_t timelineIdx, const std::vector<uint64_t> &clipIds, int64_t posMoved, int64_t durationMoved, int64_t layerMoved) noexcept {

        esotereel_gui_helper::req_clip_move_mul(
            timelineIdx,
            clipIds.data(),
            clipIds.size(),
            posMoved,
            durationMoved,
            layerMoved);
    }

    static void addClipAt(uint64_t timelineIdx, int64_t position, size_t layerIdx) noexcept {
        esotereel_gui_helper::req_add_clip_dummy(timelineIdx, position, layerIdx);
    }
};