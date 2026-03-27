#pragma once

#include "clip.h"
#include "nomyoedit_gui_helper.h"
#include <iterator>

using RawLayer = nomyoedit_gui_helper::Layer;
using RawClipIterator = nomyoedit_gui_helper::ClipIterator;
class MClipsIterator {
    nomyoedit_gui_helper::ClipIterator *rust_iter_ptr;
    const nomyoedit_gui_helper::Clip *cur_ptr;

  public:
    using iterator_category = std::forward_iterator_tag;
    using value_type = MClip;
    // ... (other traits)

    // begin用
    MClipsIterator(const RawLayer *t) noexcept
        : rust_iter_ptr(nomyoedit_gui_helper::layer_clips_begin(t)), cur_ptr(nullptr) {
        advance();
    }

    // end用
    MClipsIterator() noexcept : rust_iter_ptr(nullptr), cur_ptr(nullptr) {}

    // コピー禁止 (重要！二重解放を防ぐ)
    // std::forward_iterator はコピー可能である必要がありますが、
    // Rustのイテレータを直接持つ場合は move だけにするか、ポインタ管理を工夫する必要があります。
    MClipsIterator(const MClipsIterator &) = delete;
    MClipsIterator &operator=(const MClipsIterator &) = delete;

    // Moveは許可
    MClipsIterator(MClipsIterator &&other) noexcept
        : rust_iter_ptr(other.rust_iter_ptr), cur_ptr(other.cur_ptr) {
        other.rust_iter_ptr = nullptr;
    }

    void advance() noexcept {
        if (rust_iter_ptr) {
            cur_ptr = nomyoedit_gui_helper::clip_iter_next(rust_iter_ptr);
            if (!cur_ptr) {
                nomyoedit_gui_helper::clip_iter_free(rust_iter_ptr);
                rust_iter_ptr = nullptr;
            }
        }
    }

    MClipsIterator &operator++() noexcept {
        advance(); // 内部で free まで完結させる
        return *this;
    }

    bool operator!=(const MClipsIterator &other) const noexcept {
        // rust_iter_ptr ではなく、指しているデータ (cur_ptr) で比較するのが確実
        return cur_ptr != other.cur_ptr;
    }

    MClip operator*() const noexcept {
        return MClip(cur_ptr);
    }

    // デストラクタ: もし途中でループを抜けても Rust 側をリークさせない
    ~MClipsIterator() {
        if (rust_iter_ptr) {
            nomyoedit_gui_helper::clip_iter_free(rust_iter_ptr);
        }
    }
};
class MClipsIterable {
    const RawLayer *raw_ptr;

  public:
    MClipsIterable(const RawLayer *p) noexcept : raw_ptr(p) {}
    bool isValid() const noexcept { return raw_ptr != nullptr; }

    size_t clipsCount() const noexcept { return nomyoedit_gui_helper::layer_get_clips_count(raw_ptr); }

    // forループの開始点
    MClipsIterator begin() const noexcept { return MClipsIterator(raw_ptr); }

    // forループの終点
    MClipsIterator end() const noexcept { return MClipsIterator(); }
};