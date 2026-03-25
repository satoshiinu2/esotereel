#pragma once

#include "clip.h"
#include "muscedit_lib.h"
#include <iterator>

using RawLayer = muscedit_lib::Layer;

class MClipsIterator {
    const RawLayer *raw_ptr;
    size_t index;

  public:
    // 必須の型定義（標準ライブラリとの互換性のため）
    using iterator_category = std::forward_iterator_tag;
    using value_type = MClipsIterator;
    using difference_type = std::ptrdiff_t;
    using pointer = MClipsIterator *;
    using reference = MClipsIterator;

    MClipsIterator(const RawLayer *t, size_t i) : raw_ptr(t), index(i) {}

    // インクリメント (++it)
    MClipsIterator &operator++() {
        index++;
        return *this;
    }

    // 比較 (it != end)
    bool operator!=(const MClipsIterator &other) const {
        return index != other.index;
    }

    // 間接参照 (*it) -> ここで LayerRef を生成して返す
    MClip operator*() const {
        return MClip(muscedit_lib::layer_get_clip(raw_ptr, index));
    }
};

using RawLayer = muscedit_lib::Layer;

class MClipsIterable {
    const RawLayer *raw_ptr;

  public:
    MClipsIterable(const RawLayer *p) : raw_ptr(p) {}
    bool isValid() const { return raw_ptr != nullptr; }

    size_t clipsCount() const { return muscedit_lib::layer_get_clips_count(raw_ptr); }

    // forループの開始点
    MClipsIterator begin() const { return MClipsIterator(raw_ptr, 0); }

    // forループの終点
    MClipsIterator end() const { return MClipsIterator(raw_ptr, clipsCount()); }
};