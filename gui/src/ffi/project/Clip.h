#pragma once

#include <cstdint>

#include "esotereel_gui_helper.h"

namespace esotereel {
using RawClip = esotereel_gui_helper::Clip;

class Clip {
  public:
    const RawClip *raw_ptr;

    Clip(const RawClip *p) noexcept;

    static Clip Empty();

    bool isValid() const noexcept;
    uint64_t id() const noexcept;
    int64_t position() const noexcept;
    int64_t duration() const noexcept;
};
} // namespace esotereel