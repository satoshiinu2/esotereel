#pragma once

#include "muscedit_lib.h"
#include <cstdint>

using RawClip = muscedit_lib::Clip;

class MClip {
    const RawClip *raw_ptr;

  public:
    MClip(const RawClip *p) : raw_ptr(p) {}

    uint32_t id() const {
        return muscedit_lib::clip_get_id(raw_ptr);
    }
    int64_t position() const {
        return muscedit_lib::clip_get_position(raw_ptr);
    }
    int64_t duration() const {
        return muscedit_lib::clip_get_duration(raw_ptr);
    }
};