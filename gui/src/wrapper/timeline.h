#pragma once

#include "muscedit_lib.h"
#include "timeline_layers.h"

using RawTimeline = muscedit_lib::Timeline;

class MTimeline {
    const RawTimeline *raw_ptr;

  public:
    MTimeline(const RawTimeline *p) : raw_ptr(p) {}
    bool isValid() const { return raw_ptr != nullptr; }

    size_t layersCount() const { return muscedit_lib::timeline_get_layers_count(raw_ptr); }

    MLayersIterable layers() const {
        return MLayersIterable(raw_ptr);
    }
};