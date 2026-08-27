#pragma once

#include "ClipsIterator.h"
#include <QString>

namespace esotereel_gui_helper {
struct Layer;
struct Clip;
struct WrapperResult;
} // namespace esotereel_gui_helper

namespace esotereel {
class Clip;

using RawLayer = esotereel_gui_helper::Layer;
using RawClip = esotereel_gui_helper::Clip;

class Timeline; // Forward declaration

class Layer {
  public:
    const RawLayer *raw_ptr;

    Layer(const RawLayer *p) noexcept : raw_ptr(p) {}

    size_t clipsCount() const noexcept;

    ClipsIterable clips(const Timeline &timeline) const noexcept;

    bool isValid() const noexcept {
        return raw_ptr != nullptr;
    }

    Clip findClipAtFrame(int64_t frame, const Timeline &timeline) const noexcept;

    QString name() const noexcept;
};
} // namespace esotereel