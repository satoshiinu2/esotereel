#include "layer_clips.h"
#include "../exception.h"
#include "clip.h"
#include "esotereel_gui_helper.h"
#include <cstdint>

using RawLayer = esotereel_gui_helper::Layer;
using RawClip = esotereel_gui_helper::Clip;
using RawClipIterator = esotereel_gui_helper::ClipIterator;

RawClipIterator *ClipsIterator::getBegin(const RawLayer *t) {
    if (!t)
        return nullptr;

    RawClipIterator *ptr_iter = nullptr;

    auto result = esotereel_gui_helper::layer_clips_begin(t, &ptr_iter);
    if (!checkWrapperResult(result)) {
        return nullptr;
    }
    return ptr_iter;
}

RawClipIterator *ClipsIterator::getBeginInRange(const RawLayer *t, int64_t startFrame, int64_t endFrame) {
    if (!t)
        return nullptr;

    RawClipIterator *ptr_iter = nullptr;

    auto result = esotereel_gui_helper::layer_clips_in_range_begin(t, startFrame, endFrame, &ptr_iter);
    if (!checkWrapperResult(result)) {
        return nullptr;
    }
    return ptr_iter;
}

void ClipsIterator::advance() noexcept {
    if (raw_iter_ptr) {
        auto result = esotereel_gui_helper::clip_iter_next(raw_iter_ptr, &raw_cur_ptr);
        if (!checkWrapperResult(result)) {
            raw_cur_ptr = nullptr;
            esotereel_gui_helper::clip_iter_free(raw_iter_ptr);
            raw_iter_ptr = nullptr;
        }
    }
}

// デストラクタ
ClipsIterator::~ClipsIterator() {
    if (raw_iter_ptr) {
        esotereel_gui_helper::clip_iter_free(raw_iter_ptr);
    }
}

size_t ClipsIterable::clipsCount() const noexcept {
    return raw_ptr ? esotereel_gui_helper::layer_get_clips_count(raw_ptr) : 0;
}
