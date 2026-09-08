#pragma once

#include "ClipsIterator.h"
#include <QString>

#include "esotereel_gui_helper.h"

namespace esotereel {
using RawFolder = esotereel_gui_helper::LayerFolder;

class LayerFolder {
  public:
    const RawFolder *raw_ptr;

    LayerFolder(const RawFolder *p) noexcept : raw_ptr(p) {}

    bool isValid() const noexcept {
        return raw_ptr != nullptr;
    }

    Clip findClipAtFrame(int64_t frame, const Timeline &timeline) const noexcept;

    QString name() const noexcept;
};
} // namespace esotereel