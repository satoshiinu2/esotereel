#pragma once

#include "../stringview.h"
#include "esotereel_gui_helper.h"
#include "layer_clips.h"
#include <QString>

using RawLayer = esotereel_gui_helper::_Layer;

class Layer {
    const RawLayer *raw_ptr;

  public:
    Layer(const RawLayer *p) noexcept : raw_ptr(p) {}

    size_t clipsCount() const noexcept { return esotereel_gui_helper::layer_get_clips_count(raw_ptr); }

    MClipsIterable clips() const noexcept {
        return MClipsIterable(raw_ptr);
    }

    Clip clipAtSlow(size_t index) const noexcept {
        return Clip(esotereel_gui_helper::layer_get_clip_at_slow(raw_ptr, index));
    }

    MClipLocation findClipAtFrame(int64_t frame, size_t layerIdx) const noexcept {
        return MClipLocation(esotereel_gui_helper::layer_find_clip_at_frame(raw_ptr, frame, layerIdx));
    }

    QString name() const noexcept {
        return StringView::toQstring(esotereel_gui_helper::layer_get_name(raw_ptr));
    }
};