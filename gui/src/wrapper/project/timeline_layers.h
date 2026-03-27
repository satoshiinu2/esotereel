#pragma once

#include "layer.h"
#include "nomyoedit_gui_helper.h"
#include <iterator>

using RawTimeline = nomyoedit_gui_helper::Timeline;

class MLayersIterator {
    const RawTimeline *raw_ptr;
    size_t index;

  public:
    // 必須の型定義（標準ライブラリとの互換性のため）
    using iterator_category = std::forward_iterator_tag;
    using value_type = MLayer;
    using difference_type = std::ptrdiff_t;
    using pointer = MLayersIterator *;
    using reference = MLayersIterator;

    MLayersIterator(const RawTimeline *t, size_t i) noexcept : raw_ptr(t), index(i) {}

    // インクリメント (++it)
    MLayersIterator &operator++() noexcept {
        index++;
        return *this;
    }

    // 比較 (it != end)
    bool operator!=(const MLayersIterator &other) const noexcept {
        return index != other.index;
    }

    // 間接参照 (*it) -> ここで LayerRef を生成して返す
    MLayer operator*() const noexcept {
        return MLayer(nomyoedit_gui_helper::timeline_get_layer_at(raw_ptr, index));
    }
};

class MLayersIterable {
    const RawTimeline *raw_ptr;

  public:
    MLayersIterable(const RawTimeline *p) noexcept : raw_ptr(p) {}
    bool isValid() const noexcept { return raw_ptr != nullptr; }

    size_t layersCount() const noexcept { return nomyoedit_gui_helper::timeline_get_layers_count(raw_ptr); }

    // forループの開始点
    MLayersIterator begin() const noexcept { return MLayersIterator(raw_ptr, 0); }

    // forループの終点
    MLayersIterator end() const noexcept { return MLayersIterator(raw_ptr, layersCount()); }
};