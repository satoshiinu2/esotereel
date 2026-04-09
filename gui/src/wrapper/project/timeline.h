#pragma once

#include "layer.h"
#include "esotereel_gui_helper.h"
#include "timeline_layers.h"
#include <cstdint>
#include <set>

using RawTimeline = esotereel_gui_helper::Timeline;

class MTimeline {
    const RawTimeline *raw_ptr;

  public:
    MTimeline(const RawTimeline *p) noexcept : raw_ptr(p) {}
    bool isValid() const noexcept { return raw_ptr != nullptr; }

    size_t layersCount() const noexcept { return esotereel_gui_helper::timeline_get_layers_count(raw_ptr); }

    MLayersIterable layers() const noexcept {
        return MLayersIterable(raw_ptr);
    }

    MLayer layerAt(size_t index) const noexcept {
        return MLayer(esotereel_gui_helper::timeline_get_layer_at(raw_ptr, index));
    }

    MClipLocation findClipById(uint64_t id) const noexcept {
        return MClipLocation(esotereel_gui_helper::timeline_find_clip_by_id(raw_ptr, id));
    }

    bool canPlaceClipAt(size_t layerIdx, int64_t position, int64_t duration, const std::set<uint64_t> &exclude_set) const {
        // そこまでsetは大きくないと信じてコピー
        std::vector<uint64_t> exclude_vec(exclude_set.begin(), exclude_set.end());
        return esotereel_gui_helper::timeline_can_place_clip_at(
            raw_ptr, layerIdx, position, duration,
            exclude_vec.data(), exclude_vec.size());
    }
};