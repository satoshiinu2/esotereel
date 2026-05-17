#pragma once

#include "esotereel_gui_helper.h"
#include "layer.h"
#include <iterator>

using RawTimeline = esotereel_gui_helper::Timeline;

class LayersIterator {
    const RawTimeline *raw_ptr;
    size_t index;

  public:
    // 必須の型定義（標準ライブラリとの互換性のため）
    using iterator_category = std::forward_iterator_tag;
    using value_type = Layer;
    using difference_type = std::ptrdiff_t;
    using pointer = LayersIterator *;
    using reference = LayersIterator;

    LayersIterator(const RawTimeline *t, size_t i) noexcept : raw_ptr(t), index(i) {
    }

    // インクリメント (++it)
    LayersIterator &operator++() noexcept {
        index++;
        return *this;
    }

    // 比較 (it != end)
    bool operator!=(const LayersIterator &other) const noexcept {
        return index != other.index;
    }

    // 間接参照 (*it) -> ここで LayerRef を生成して返す
    Layer operator*() const noexcept {
        return Layer(esotereel_gui_helper::timeline_get_layer_by_layer_handle(raw_ptr, index));
    }
};

class LayersIterable {
    const RawTimeline *raw_ptr;

  public:
    LayersIterable(const RawTimeline *p) noexcept : raw_ptr(p) {
    }
    bool isValid() const noexcept {
        return raw_ptr != nullptr;
    }

    size_t layersCount() const noexcept {
        return esotereel_gui_helper::timeline_get_layers_count(raw_ptr);
    }

    // forループの開始点
    LayersIterator begin() const noexcept {
        return LayersIterator(raw_ptr, 0);
    }

    // forループの終点
    LayersIterator end() const noexcept {
        return LayersIterator(raw_ptr, layersCount());
    }
};