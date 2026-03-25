#pragma once

#include "layer.h"
#include "muscedit_lib.h"
#include <iterator>

using RawTimeline = muscedit_lib::Timeline;

class MLayersIterator {
    const RawTimeline *raw_ptr;
    size_t index;

  public:
    // 必須の型定義（標準ライブラリとの互換性のため）
    using iterator_category = std::forward_iterator_tag;
    using value_type = MLayersIterator;
    using difference_type = std::ptrdiff_t;
    using pointer = MLayersIterator *;
    using reference = MLayersIterator;

    MLayersIterator(const RawTimeline *t, size_t i) : raw_ptr(t), index(i) {}

    // インクリメント (++it)
    MLayersIterator &operator++() {
        index++;
        return *this;
    }

    // 比較 (it != end)
    bool operator!=(const MLayersIterator &other) const {
        return index != other.index;
    }

    // 間接参照 (*it) -> ここで LayerRef を生成して返す
    MLayer operator*() const {
        return MLayer(muscedit_lib::timeline_get_layer(raw_ptr, index));
    }
};

using RawTimeline = muscedit_lib::Timeline;

class MLayersIterable {
    const RawTimeline *raw_ptr;

  public:
    MLayersIterable(const RawTimeline *p) : raw_ptr(p) {}
    bool isValid() const { return raw_ptr != nullptr; }

    size_t layersCount() const { return muscedit_lib::timeline_get_layers_count(raw_ptr); }

    // forループの開始点
    MLayersIterator begin() const { return MLayersIterator(raw_ptr, 0); }

    // forループの終点
    MLayersIterator end() const { return MLayersIterator(raw_ptr, layersCount()); }
};