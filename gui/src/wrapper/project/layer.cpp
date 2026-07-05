#include "layer.h"
#include "../exception.h"
#include "../stringview.h"
#include "esotereel_gui_helper.h"
#include "layer_clips.h"
#include <QString>

using RawLayer = esotereel_gui_helper::Layer;
using RawClip = esotereel_gui_helper::Clip;
using WrapperResult = esotereel_gui_helper::WrapperResult;

size_t Layer::clipsCount() const noexcept {
    return esotereel_gui_helper::layer_get_clips_count(raw_ptr);
}

ClipsIterable Layer::clips() const noexcept {
    return ClipsIterable(raw_ptr);
}

Clip Layer::findClipAtFrame(int64_t frame) const noexcept {
    const RawClip *clip;

    auto result = esotereel_gui_helper::layer_find_clip_at_frame(raw_ptr, frame, &clip);
    if (!checkWrapperResult(result)) {
        return Clip::Empty();
    }

    return Clip(clip);
}

QString Layer::name() const noexcept {
    return StringView::toQstring(esotereel_gui_helper::layer_get_name(raw_ptr));
}
