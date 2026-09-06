#pragma once

#include "Clip.h"
#include <cstdint>
#include <iterator>

namespace esotereel_gui_helper {
struct Layer;
struct Clip;
struct Timeline;
} // namespace esotereel_gui_helper

namespace esotereel {
using RawLayer = esotereel_gui_helper::Layer;
using RawClip = esotereel_gui_helper::Clip;
using RawTimeline = esotereel_gui_helper::Timeline;

class Timeline; // Forward declaration

class ClipsIterator {
    const RawLayer *raw_layer_ptr;
    const RawTimeline *timeline_ptr;
    const RawClip *raw_cur_ptr;
    size_t current_index;
    size_t total_count;

    void initialize() noexcept;

  public:
    using iterator_category = std::forward_iterator_tag;
    using value_type = Clip;
    using difference_type = std::ptrdiff_t;
    using pointer = ClipsIterator *;
    using reference = ClipsIterator;

    // begin用
    ClipsIterator(const RawLayer *t, const RawTimeline *timeline) noexcept;

    ClipsIterator(const RawLayer *t, const RawTimeline *timeline, int64_t startFrame, int64_t endFrame) noexcept;

    // end用
    ClipsIterator() noexcept
        : raw_layer_ptr(nullptr), timeline_ptr(nullptr), raw_cur_ptr(nullptr), current_index(0), total_count(0) {}

    // コピー許可（index-basedなので安全）
    ClipsIterator(const ClipsIterator &other) noexcept
        : raw_layer_ptr(other.raw_layer_ptr), timeline_ptr(other.timeline_ptr), raw_cur_ptr(other.raw_cur_ptr),
          current_index(other.current_index), total_count(other.total_count) {}

    ClipsIterator &operator=(const ClipsIterator &other) noexcept {
        if (this != &other) {
            raw_layer_ptr = other.raw_layer_ptr;
            timeline_ptr = other.timeline_ptr;
            raw_cur_ptr = other.raw_cur_ptr;
            current_index = other.current_index;
            total_count = other.total_count;
        }
        return *this;
    }

    void advance() noexcept;

    ClipsIterator &operator++() noexcept {
        advance();
        return *this;
    }

    bool operator!=(const ClipsIterator &other) const noexcept {
        return raw_cur_ptr != other.raw_cur_ptr;
    }

    bool operator==(const ClipsIterator &other) const noexcept {
        return raw_cur_ptr == other.raw_cur_ptr;
    }

    Clip operator*() const noexcept {
        return Clip(raw_cur_ptr);
    }

    // デストラクタ
    ~ClipsIterator();
};

class ClipsIterable {
    const RawLayer *raw_ptr;
    const RawTimeline *timeline_ptr;

  public:
    ClipsIterable(const RawLayer *p, const Timeline &timeline) noexcept;
    bool isValid() const noexcept {
        return raw_ptr != nullptr;
    }

    size_t clipsCount() const noexcept;

    // forループの開始点
    ClipsIterator begin() const noexcept {
        return ClipsIterator(raw_ptr, timeline_ptr);
    }

    // forループの終点
    ClipsIterator end() const noexcept {
        return ClipsIterator();
    }

    // 範囲指定イテレータ
    ClipsIterator range(int64_t startFrame, int64_t endFrame) const noexcept {
        return ClipsIterator(raw_ptr, timeline_ptr, startFrame, endFrame);
    }
};
} // namespace esotereel