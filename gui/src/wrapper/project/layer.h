#pragma once

#include "../stringview.h"
#include "esotereel_gui_helper.h"
#include "layer_clips.h"
#include <QString>

using RawLayer = esotereel_gui_helper::_Layer;
using RawClip = esotereel_gui_helper::_Clip;
using WrapperErrorCode = esotereel_gui_helper::_WrapperErrorCode;

class Layer {
  public:
    const RawLayer *raw_ptr;

    Layer(const RawLayer *p) noexcept : raw_ptr(p) {
    }

    size_t clipsCount() const noexcept {
        return esotereel_gui_helper::layer_get_clips_count(raw_ptr);
    }

    ClipsIterable clips() const noexcept {
        return ClipsIterable(raw_ptr);
    }

    Clip findClipAtFrame(int64_t frame) const noexcept {
        const RawClip *clip;
        auto resylt = esotereel_gui_helper::layer_find_clip_at_frame(raw_ptr, frame, &clip);
        if (resylt != WrapperErrorCode::Ok) {
            return Clip::Empty();
        }

        return Clip(clip);
    }

    QString name() const noexcept {
        return StringView::toQstring(esotereel_gui_helper::layer_get_name(raw_ptr));
    }
};