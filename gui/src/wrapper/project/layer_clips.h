#pragma once

#include "clip.h"
#include "esotereel_gui_helper.h"
#include <iterator>

using RawLayer = esotereel_gui_helper::_Layer;
using RawClip = esotereel_gui_helper::_Clip;
using RawClipIterator = esotereel_gui_helper::_ClipIterator;

class ClipsIterator {
    RawClipIterator *rust_iter_ptr;
    const RawClip *cur_ptr;

  public:
    using iterator_category = std::forward_iterator_tag;
    using value_type = Clip;
    // ... (other traits)

    // begin用
    ClipsIterator(const RawLayer *t) noexcept
        : rust_iter_ptr(esotereel_gui_helper::layer_clips_begin(t)), cur_ptr(nullptr) {
        advance();
    }

    // end用
    ClipsIterator() noexcept : rust_iter_ptr(nullptr), cur_ptr(nullptr) {}

    // コピー禁止 (重要！二重解放を防ぐ)
    // std::forward_iterator はコピー可能である必要がありますが、
    // Rustのイテレータを直接持つ場合は move だけにするか、ポインタ管理を工夫する必要があります。
    ClipsIterator(const ClipsIterator &) = delete;
    ClipsIterator &operator=(const ClipsIterator &) = delete;

    // Moveは許可
    ClipsIterator(ClipsIterator &&other) noexcept
        : rust_iter_ptr(other.rust_iter_ptr), cur_ptr(other.cur_ptr) {
        other.rust_iter_ptr = nullptr;
    }

    void advance() noexcept {
        if (rust_iter_ptr) {
            cur_ptr = esotereel_gui_helper::clip_iter_next(rust_iter_ptr);
            if (!cur_ptr) {
                esotereel_gui_helper::clip_iter_free(rust_iter_ptr);
                rust_iter_ptr = nullptr;
            }
        }
    }

    ClipsIterator &operator++() noexcept {
        advance(); // 内部で free まで完結させる
        return *this;
    }

    bool operator!=(const ClipsIterator &other) const noexcept {
        // rust_iter_ptr ではなく、指しているデータ (cur_ptr) で比較するのが確実
        return cur_ptr != other.cur_ptr;
    }

    Clip operator*() const noexcept {
        return Clip(cur_ptr);
    }

    // デストラクタ: もし途中でループを抜けても Rust 側をリークさせない
    ~ClipsIterator() {
        if (rust_iter_ptr) {
            esotereel_gui_helper::clip_iter_free(rust_iter_ptr);
        }
    }
};
class MClipsIterable {
    const RawLayer *raw_ptr;

  public:
    MClipsIterable(const RawLayer *p) noexcept : raw_ptr(p) {}
    bool isValid() const noexcept { return raw_ptr != nullptr; }

    size_t clipsCount() const noexcept { return esotereel_gui_helper::layer_get_clips_count(raw_ptr); }

    // forループの開始点
    ClipsIterator begin() const noexcept { return ClipsIterator(raw_ptr); }

    // forループの終点
    ClipsIterator end() const noexcept { return ClipsIterator(); }
};