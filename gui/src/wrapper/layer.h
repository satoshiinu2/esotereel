#pragma once

#include "layer_clips.h"
#include "muscedit_lib.h"
#include <QString>

using RawLayer = muscedit_lib::Layer;

class MLayer {
    const RawLayer *raw_ptr;

  public:
    MLayer(const RawLayer *p) : raw_ptr(p) {}

    size_t clipsCount() const { return muscedit_lib::layer_get_clips_count(raw_ptr); }

    MClipsIterable clips() const {
        return MClipsIterable(raw_ptr);
    }

    QString name() const {
        const char *p = (const char *)muscedit_lib::layer_get_name_ptr(raw_ptr);
        size_t len = muscedit_lib::layer_get_name_len(raw_ptr);

        return QString::fromUtf8(p, (int)len);
    }
};