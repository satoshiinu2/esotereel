#include "Timeline.h"
#include "Clip.h"
#include "Layer.h"
#include "LayerFolder.h"
#include "LayersIterator.h"
#include "esotereel_gui_helper.h"
#include "ffi/Requests.h"
#include "ffi/Result.h"
#include "ffi/WrapperResult.h"
#include <cmath>
#include <cstdint>
#include <vector>

namespace esotereel {
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

Layer Timeline::layerById(LayerId layer_id) const noexcept {
    return Layer(esotereel_gui_helper::timeline_get_layer_by_id(raw_ptr, layer_id));
}

LayerFolder Timeline::folderById(LayerFolderId folder_id) const noexcept {
    return LayerFolder(esotereel_gui_helper::timeline_get_folder_by_id(raw_ptr, folder_id));
}

std::tuple<Clip, LayerId> Timeline::findClipById(ClipId id) const noexcept {
    const esotereel_gui_helper::Clip *raw_clip = nullptr;
    uint64_t layerId = 0;

    auto result = esotereel_gui_helper::timeline_find_clip_by_id(raw_ptr, id, &raw_clip, &layerId);
    if (!checkWrapperResult(result)) {
        return std::make_tuple(Clip::Empty(), 0);
    }
    return std::make_tuple(Clip(raw_clip), layerId);
}

bool Timeline::canPlaceClipAt(uint64_t layer_id, int64_t position, int64_t duration,
                              const std::set<uint64_t> &exclude_set) const {
    if (!raw_ptr)
        return false;

    // そこまでsetは大きくないと信じてコピー
    std::vector<uint64_t> exclude_vec(exclude_set.begin(), exclude_set.end());

    return esotereel_gui_helper::timeline_can_place_clip_at(raw_ptr, layer_id, position, duration, exclude_vec.data(),
                                                            exclude_vec.size());
}

double_t Timeline::fps() const noexcept {
    return esotereel_gui_helper::timeline_get_fps(raw_ptr);
}
} // namespace esotereel