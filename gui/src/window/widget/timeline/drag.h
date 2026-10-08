#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <qpoint.h>
#include <variant>

namespace esotereel::window {

// Forward declaration - full definition in resize.h
struct DragClipResize;

struct DragNone {};
struct DragOther {};

struct DragClip {
    size_t srcLayerIdx;
    int64_t srcFrame;
    size_t curLayerIdx;
    int64_t curFrame;
    QPointF ghostPos;
    bool isWrong;
};

struct DragAreaSel {
    QPointF start;
    QPointF current;
};

struct DragPlayHead {};

using DragState = std::variant<DragNone, DragOther, DragClip, DragClipResize, DragAreaSel, DragPlayHead>;

} // namespace esotereel::window
