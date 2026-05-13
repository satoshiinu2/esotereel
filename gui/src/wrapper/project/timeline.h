#pragma once

#include <cmath>
#include <cstddef>
#include <cstdint>
#include <set>
#include <tuple>

namespace esotereel_gui_helper {
struct _Timeline;
}
using RawTimeline = esotereel_gui_helper::_Timeline;

class Clip;
class Layer;
class LayersIterable;

class Timeline {
    const RawTimeline *raw_ptr;

  public:
    Timeline(const RawTimeline *p) noexcept;

    operator const RawTimeline *() const noexcept {
        return raw_ptr;
    }

    bool isValid() const noexcept;
    size_t layersCount() const noexcept;
    LayersIterable layers() const noexcept;

    Layer layerByLayerHandle(size_t layer_handle) const noexcept;
    Layer layerSortedAt(uint32_t index) const noexcept;

    std::tuple<Clip, size_t> findClipById(uint64_t id) const noexcept;
    bool canPlaceClipAt(uint32_t layerIdx, int64_t position, int64_t duration,
                        const std::set<uint64_t> &exclude_set) const;

    double_t fps() const noexcept;
};