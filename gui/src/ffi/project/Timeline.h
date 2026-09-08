#pragma once

#include "ffi/Requests.h"
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <set>
#include <tuple>

#include "esotereel_gui_helper.h"

namespace esotereel {
using RawTimeline = esotereel_gui_helper::Timeline;
using LayerId = esotereel_gui_helper::LayerId;
using LayerFolderId = esotereel_gui_helper::LayerFolderId;
using ClipId = esotereel_gui_helper::ClipId;
using TimelineTick = esotereel_gui_helper::TimelineTick;

// fwd
class Clip;
class Layer;
class LayerFolder;
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

    Layer layerById(LayerId layer_id) const noexcept;
    LayerFolder folderById(LayerFolderId folder_id) const noexcept;

    std::tuple<Clip, LayerId> findClipById(ClipId id) const noexcept;
    bool canPlaceClipAt(LayerId layer_id, TimelineTick position, TimelineTick duration,
                        const std::set<ClipId> &exclude_set) const;

    double_t fps() const noexcept;
};

} // namespace esotereel