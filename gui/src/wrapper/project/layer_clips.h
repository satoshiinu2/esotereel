#pragma once

#include "../exception.h"
#include "clip.h"
#include <cstdint>
#include <iterator>

namespace esotereel_gui_helper {
    struct Layer;
    struct Clip;
    struct ClipIterator;
    struct WrapperResult;
}

using RawLayer = esotereel_gui_helper::Layer;
using RawClip = esotereel_gui_helper::Clip;
using RawClipIterator = esotereel_gui_helper::ClipIterator;
using WrapperResult = esotereel_gui_helper::WrapperResult;

class ClipsIterator {
    RawClipIterator *raw_iter_ptr;
    const RawClip *raw_cur_ptr;

    static RawClipIterator *getBegin(const RawLayer *t) ;

    static RawClipIterator *getBeginInRange(const RawLayer *t, int64_t startFrame, int64_t endFrame);

  public:
    using iterator_category = std::forward_iterator_tag;
    using value_type = Clip;
    using difference_type = std::ptrdiff_t;
    using pointer = ClipsIterator *;
    using reference = ClipsIterator;

    // begin用
    ClipsIterator(const RawLayer *t) noexcept : raw_iter_ptr(ClipsIterator::getBegin(t)), raw_cur_ptr(nullptr) {
        advance();
    }

    ClipsIterator(const RawLayer *t, int64_t startFrame, int64_t endFrame) noexcept
        : raw_iter_ptr(ClipsIterator::getBeginInRange(t, startFrame, endFrame)), raw_cur_ptr(nullptr) {
        advance();
    }

    // end用
    ClipsIterator() noexcept : raw_iter_ptr(nullptr), raw_cur_ptr(nullptr) {}

    // コピー禁止
    ClipsIterator(const ClipsIterator &) = delete;
    ClipsIterator &operator=(const ClipsIterator &) = delete;

    // Moveは許可
    ClipsIterator(ClipsIterator &&other) noexcept : raw_iter_ptr(other.raw_iter_ptr), raw_cur_ptr(other.raw_cur_ptr) {
        other.raw_iter_ptr = nullptr;
    }

    void advance() noexcept ;

    ClipsIterator &operator++() noexcept {
        advance(); // 内部で free まで完結させる
        return *this;
    }

    bool operator!=(const ClipsIterator &other) const noexcept {
        return raw_cur_ptr != other.raw_cur_ptr;
    }

    Clip operator*() const noexcept {
        return Clip(raw_cur_ptr);
    }

    // デストラクタ
    ~ClipsIterator();
};

class ClipsIterable {
    const RawLayer *raw_ptr;

  public:
    ClipsIterable(const RawLayer *p) noexcept : raw_ptr(p) {}
    bool isValid() const noexcept {
        return raw_ptr != nullptr;
    }

    size_t clipsCount() const noexcept ;

    // forループの開始点
    ClipsIterator begin() const noexcept {
        return ClipsIterator(raw_ptr);
    }

    // forループの終点
    ClipsIterator end() const noexcept {
        return ClipsIterator();
    }
};