#pragma once

#include <cmath>
#include <cstddef>
#include <cstdint>
#include <set>
#include <tuple>

namespace esotereel_gui_helper {
struct Timeline;
}
namespace esotereel {
using RawTimeline = esotereel_gui_helper::Timeline;

class Clip;
class Layer;
class LayersIterable;

class Timeline {
  public:
    const RawTimeline *raw_ptr;

    Timeline(const RawTimeline *p = nullptr) noexcept;

    operator const RawTimeline *() const noexcept {
        return raw_ptr;
    }

    bool isValid() const noexcept;
    size_t layersCount() const noexcept;
    LayersIterable layers() const noexcept;

    Layer layerByLayerHandle(size_t layer_handle) const noexcept;
    Layer layerSortedAt(uint32_t index) const noexcept;
    Layer layerById(uint64_t layer_id) const noexcept;
    uint64_t layerIdAtRootIndex(size_t index) const noexcept;

    std::tuple<Clip, uint64_t> findClipById(uint64_t id) const noexcept;
    bool canPlaceClipAt(uint64_t layer_id, int64_t position, int64_t duration,
                        const std::set<uint64_t> &exclude_set) const;

    double_t fps() const noexcept;
};

} // namespace esotereel