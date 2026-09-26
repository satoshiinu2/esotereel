#include "Layer.h"
#include "ClipsIterator.h"
#include "Timeline.h"
#include "esotereel_gui_helper.h"
#include "ffi/StringView.h"
#include "ffi/WrapperResult.h"
#include <QString>

namespace esotereel {
using RawLayer = esotereel_gui_helper::Layer;
using RawClip = esotereel_gui_helper::Clip;
using RawTimeline = esotereel_gui_helper::Timeline;

size_t Layer::clipsCount() const noexcept {
    return esotereel_gui_helper::layer_get_clips_count(raw_ptr);
}

ClipsIterable Layer::clips(const Timeline &timeline) const noexcept {
    return ClipsIterable(raw_ptr, timeline);
}

Clip Layer::findClipAtFrame(int64_t frame, const Timeline &timeline) const noexcept {
    auto result = esotereel_gui_helper::layer_find_clip_at_frame(raw_ptr, timeline.raw_ptr, frame);
    if (!result.is_ok) {
        return Clip::Empty();
    }

    return Clip(result.value.ok);
}

QString Layer::name() const noexcept {
    auto result = esotereel_gui_helper::layer_get_name(raw_ptr);
    if (!result.is_ok) {
        return QString(); // Return empty string on error
    }

    return StringView::toQString(result.value.ok);
}
} // namespace esotereel