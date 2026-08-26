#include "timeline_layers.h"
#include "esotereel_gui_helper.h"

using RawTimeline = esotereel_gui_helper::Timeline;

// 間接参照 (*it) -> ここで LayerRef を生成して返す
Layer LayersIterator::operator*() const noexcept {
    // 新しいデータモデルでは、インデックスからLayerIdを取得してからLayerを取得する
    uint64_t layer_id = esotereel_gui_helper::timeline_get_layer_id_at_root_index(raw_ptr, index);
    return Layer(esotereel_gui_helper::timeline_get_layer_by_id(raw_ptr, layer_id));
}

size_t LayersIterable::layersCount() const noexcept {
    return esotereel_gui_helper::timeline_get_layers_count(raw_ptr);
}
