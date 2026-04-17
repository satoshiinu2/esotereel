#pragma once

#include "esotereel_gui_helper.h"
#include <cstddef>
#include <cstdint>

using RawClip = esotereel_gui_helper::_Clip;
using RawClipLocation = esotereel_gui_helper::_ClipLocation;

class Clip {
    const RawClip *raw_ptr;

  public:
    Clip(const RawClip *p) noexcept : raw_ptr(p) {}
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
class MClipLocation {
  private:
    MClipLocation() : clip(Clip(nullptr)), clipIdx(0), layerIdx(0) {}

  public:
    const Clip clip;
    const size_t clipIdx;
    const size_t layerIdx;

    static MClipLocation Empty() {
        return MClipLocation();
    }
    MClipLocation(const RawClipLocation &raw) : clip(Clip(raw.clip)), clipIdx(raw.clip_idx), layerIdx(raw.layer_idx) {}
    bool isValid() const noexcept { return clip.isValid(); }
};