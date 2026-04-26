#pragma once

#include "esotereel_gui_helper.h"
#include <cstdint>

using RawClip = esotereel_gui_helper::_Clip;

class Clip {
  public:
    const RawClip *raw_ptr;

    Clip(const RawClip *p) noexcept : raw_ptr(p) {}
    static Clip Empty() {
        return Clip(nullptr);
    }

    bool isValid() const noexcept { return raw_ptr != nullptr; }

    uint64_t id() const noexcept {
        return esotereel_gui_helper::clip_get_id(raw_ptr);
    }
    int64_t position() const noexcept {
        return esotereel_gui_helper::clip_get_position(raw_ptr);
    }
    int64_t duration() const noexcept {
        return esotereel_gui_helper::clip_get_duration(raw_ptr);
    }
};