#include "timeline_layers.h"
#include "esotereel_gui_helper.h"

using RawTimeline = esotereel_gui_helper::Timeline;

// 間接参照 (*it) -> ここで LayerRef を生成して返す
Layer LayersIterator::operator*() const noexcept {
    return Layer(esotereel_gui_helper::timeline_get_layer_by_order(raw_ptr, index));
}

size_t LayersIterable::layersCount() const noexcept {
    return esotereel_gui_helper::timeline_get_layers_count(raw_ptr);
}
