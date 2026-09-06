#include "Clip.h"
#include "esotereel_gui_helper.h"

namespace esotereel {
Clip::Clip(const RawClip *p) noexcept : raw_ptr(p) {}

Clip Clip::Empty() {
    return Clip(nullptr);
}

bool Clip::isValid() const noexcept {
    return raw_ptr != nullptr;
}

uint64_t Clip::id() const noexcept {
    return esotereel_gui_helper::clip_get_id(raw_ptr);
}

int64_t Clip::position() const noexcept {
    return esotereel_gui_helper::clip_get_position(raw_ptr);
}

int64_t Clip::duration() const noexcept {
    return esotereel_gui_helper::clip_get_duration(raw_ptr);
}
} // namespace esotereel