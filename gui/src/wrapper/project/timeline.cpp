#include "timeline.h"
#include "clip.h"
#include "esotereel_gui_helper.h"
#include "layer.h"
#include "timeline_layers.h"
#include <cmath>
#include <vector>

using WrapperErrorCode = esotereel_gui_helper::WrapperErrorCode;

Timeline::Timeline(const RawTimeline *p) noexcept : raw_ptr(p) {}

bool Timeline::isValid() const noexcept {
    return raw_ptr != nullptr;
}

size_t Timeline::layersCount() const noexcept {
    return esotereel_gui_helper::timeline_get_layers_count(raw_ptr);
}

LayersIterable Timeline::layers() const noexcept {
    return LayersIterable(raw_ptr);
}

Layer Timeline::layerByLayerHandle(size_t layer_handle) const noexcept {
    return Layer(esotereel_gui_helper::timeline_get_layer_by_layer_handle(raw_ptr, layer_handle));
}

Layer Timeline::layerSortedAt(uint32_t index) const noexcept {
    return Layer(esotereel_gui_helper::timeline_get_layer_by_sorted_idx(raw_ptr, index));
}

std::tuple<Clip, size_t> Timeline::findClipById(uint64_t id) const noexcept {
    const esotereel_gui_helper::Clip *raw_clip;
    size_t layerIdx;

    auto result = esotereel_gui_helper::timeline_find_clip_by_id(raw_ptr, id, &raw_clip, &layerIdx);

    if (result != WrapperErrorCode::Ok) {
        return std::make_tuple(Clip::Empty(), 0);
    }
    return std::make_tuple(Clip(raw_clip), layerIdx);
}

bool Timeline::canPlaceClipAt(uint32_t layerIdx, int64_t position, int64_t duration,
                              const std::set<uint64_t> &exclude_set) const {
    if (!raw_ptr)
        return false;

    // そこまでsetは大きくないと信じてコピー
    std::vector<uint64_t> exclude_vec(exclude_set.begin(), exclude_set.end());

    return esotereel_gui_helper::timeline_can_place_clip_at(raw_ptr, layerIdx, position, duration, exclude_vec.data(),
                                                            exclude_vec.size());
}

double_t Timeline::fps() const noexcept {
    return esotereel_gui_helper::timeline_get_fps(raw_ptr);
}