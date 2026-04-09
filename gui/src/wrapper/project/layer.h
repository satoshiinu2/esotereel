#pragma once

#include "layer_clips.h"
#include "esotereel_gui_helper.h"
#include <QString>

using RawLayer = esotereel_gui_helper::Layer;

class MLayer {
    const RawLayer *raw_ptr;

  public:
    MLayer(const RawLayer *p) noexcept : raw_ptr(p) {}

    size_t clipsCount() const noexcept { return esotereel_gui_helper::layer_get_clips_count(raw_ptr); }

    MClipsIterable clips() const noexcept {
        return MClipsIterable(raw_ptr);
    }

    MClip clipAtSlow(size_t index) const noexcept {
        return MClip(esotereel_gui_helper::layer_get_clip_at_slow(raw_ptr, index));
    }

    MClipLocation findClipAtFrame(int64_t frame, size_t layerIdx) const noexcept {
        return MClipLocation(esotereel_gui_helper::layer_find_clip_at_frame(raw_ptr, frame, layerIdx));
    }

    QString name() const noexcept {
        const char *p = (const char *)esotereel_gui_helper::layer_get_name_ptr(raw_ptr);
        size_t len = esotereel_gui_helper::layer_get_name_len(raw_ptr);

        return QString::fromUtf8(p, (int)len);
    }
};