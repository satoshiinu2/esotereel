#pragma once

#include "clip.h"
#include "esotereel_gui_helper.h"
#include "layer.h"
#include "timeline_layers.h"
#include <cstddef>
#include <cstdint>
#include <set>
#include <tuple>

using RawTimeline = esotereel_gui_helper::_Timeline;
using WrapperErrorCode = esotereel_gui_helper::_WrapperErrorCode;

class Timeline {
  public:
    const RawTimeline *raw_ptr;

    Timeline(const RawTimeline *p) noexcept : raw_ptr(p) {}
    bool isValid() const noexcept { return raw_ptr != nullptr; }

    size_t layersCount() const noexcept {
        return esotereel_gui_helper::timeline_get_layers_count(raw_ptr);
    }

    LayersIterable layers() const noexcept {
        return LayersIterable(raw_ptr);
    }

    Layer layerAt(size_t index) const noexcept {
        return Layer(esotereel_gui_helper::timeline_get_layer_at(raw_ptr, index));
    }

    std::tuple<Clip, size_t> findClipById(uint64_t id) const noexcept {
        const RawClip *raw_clip;
        size_t layerIdx;
        auto result = esotereel_gui_helper::timeline_find_clip_by_id(raw_ptr, id, &raw_clip, &layerIdx);
        if (result != WrapperErrorCode::Ok) {
            return std::make_tuple(Clip::Empty(), 0);
        }
        return std::make_tuple(Clip(raw_clip), layerIdx);
    }

    bool canPlaceClipAt(size_t layerIdx, int64_t position, int64_t duration, const std::set<uint64_t> &exclude_set) const {
        // そこまでsetは大きくないと信じてコピー
        std::vector<uint64_t> exclude_vec(exclude_set.begin(), exclude_set.end());
        return esotereel_gui_helper::timeline_can_place_clip_at(
            raw_ptr, layerIdx, position, duration,
            exclude_vec.data(), exclude_vec.size());
    }
};