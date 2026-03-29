#pragma once

#include "nomyoedit_gui_helper.h"
#include <cstddef>
#include <cstdint>

using RawClip = nomyoedit_gui_helper::Clip;
using RawClipLocation = nomyoedit_gui_helper::ClipLocation;

class MClip {
    const RawClip *raw_ptr;

  public:
    MClip(const RawClip *p) noexcept : raw_ptr(p) {}
    bool isValid() const noexcept { return raw_ptr != nullptr; }

    uint64_t id() const noexcept {
        return nomyoedit_gui_helper::clip_get_id(raw_ptr);
    }
    int64_t position() const noexcept {
        return nomyoedit_gui_helper::clip_get_position(raw_ptr);
    }
    int64_t duration() const noexcept {
        return nomyoedit_gui_helper::clip_get_duration(raw_ptr);
    }
};
class MClipLocation {
  public:
    const MClip clip;
    const size_t clipIdx;
    const size_t layerIdx;

    MClipLocation() : clip(MClip(nullptr)), clipIdx(0), layerIdx(0) {}
    MClipLocation(const RawClipLocation &raw) : clip(MClip(raw.clip)), clipIdx(raw.clip_idx), layerIdx(raw.layer_idx) {}
    bool isValid() const noexcept { return clip.isValid(); }
};