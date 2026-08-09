#include "../../util.h"
#include "../../wrapper/network.h"
#include "../../wrapper/project/clip.h"            // IWYU pragma: keep
#include "../../wrapper/project/layer.h"           // IWYU pragma: keep
#include "../../wrapper/project/layer_clips.h"     // IWYU pragma: keep
#include "../../wrapper/project/project.h"         // IWYU pragma: keep
#include "../../wrapper/project/timeline.h"        // IWYU pragma: keep
#include "../../wrapper/project/timeline_layers.h" // IWYU pragma: keep
#include "../../wrapper/requests.h"
#include "../main.h"
#include "esotereel_gui_helper.h"
#include "timeline.h"
#include <QColor>
#include <QEvent>
#include <QPainter>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>

std::optional<DragClip> TimelineWidget::handleClipDragGrab(const Project &project, const Timeline &timeline,
                                                           const QPoint &mousePos, bool ctrl) {
    int64_t frame = this->XToFrame(mousePos.x());

    auto [clip, layerIdx] = this->findClipAt(project, timeline, mousePos);
    if (!clip.isValid()) {
        return std::nullopt;
    }

    // range check]

    if (!contains(this->selectedClipIds, clip.id())) {
        if (!ctrl) {
            this->selectedClipIds.clear();
        }
        this->selectedClipIds.insert(clip.id());
    }

    update();

    return DragClip{layerIdx, frame, layerIdx, frame, mousePos, false};
}

void TimelineWidget::handleClipDragContinue(const Timeline &timeline, const QPoint &mousePos) {
    int64_t frame = this->XToFrame(mousePos.x());
    ssize_t layerIdx = this->YToRow(mousePos.y());

    auto *drag = std::get_if<DragClip>(&this->dragState);
    if (!drag) {
        return;
    }

    drag->curFrame = frame;
    drag->curLayerIdx = layerIdx;
    drag->ghostPos = mousePos;

    // ovetlap check
    int64_t frameMoved = drag->curFrame - drag->srcFrame;
    int32_t layerMoved = drag->curLayerIdx - drag->srcLayerIdx;

    drag->isWrong = false;
    for (uint64_t clipid : this->selectedClipIds) {
        auto [clip, layerIdx] = timeline.findClipById(clipid);
        if (!clip.isValid()) {
            continue;
        }

        uint32_t targetLayerIdx = (layerIdx + layerMoved);
        int64_t newClipPosition = clip.position() + frameMoved;

        if (!timeline.canPlaceClipAt(targetLayerIdx, newClipPosition, clip.duration(), this->selectedClipIds)) {
            auto *clipDragState = std::get_if<DragClip>(&this->dragState);
            if (!clipDragState) {
                return;
            }
            clipDragState->isWrong = true;
            goto finalize;
        }
    }

finalize:
    update();
}

void TimelineWidget::handleClipDraggingDrop(const Timeline &timeline, const QPoint &mousePos) {
    handleClipDragContinue(timeline, mousePos);

    auto *drag = std::get_if<DragClip>(&this->dragState);
    if (!drag) {
        return;
    }

    int64_t frameMoved = drag->curFrame - drag->srcFrame;
    int32_t layerMoved = drag->curLayerIdx - drag->srcLayerIdx;
    // range and overrap check
    for (uint64_t clipId : this->selectedClipIds) {
        auto [clip, layerIdx] = timeline.findClipById(clipId);
        if (!clip.isValid()) {
            continue;
        }
        uint32_t targetLayerIdx = (layerIdx + layerMoved);
        int64_t newClipPosition = clip.position() + frameMoved;
        if (!timeline.canPlaceClipAt(targetLayerIdx, newClipPosition, clip.duration(), this->selectedClipIds)) {
            goto send_drop;
        }
    }

send_drop:
    std::vector<uint64_t> exclude_vec(this->selectedClipIds.begin(), this->selectedClipIds.end());

    this->windowState.network->requests().moveClips(this->timelineIdx, exclude_vec, frameMoved, 0, layerMoved);

    this->markRowsDirty();
    update();
}

void TimelineWidget::drawDragGhost(const Timeline &timeline, QPainter &p, const QRect &r) const {
    auto *drag = std::get_if<DragClip>(&this->dragState);
    if (!drag) {
        return;
    }

    int64_t frameMoved = drag->curFrame - drag->srcFrame;
    int64_t layerMoved = drag->curLayerIdx - drag->srcLayerIdx;
    // range and overrap check
    for (uint64_t clipId : this->selectedClipIds) {
        auto [clip, layerIdx] = timeline.findClipById(clipId);
        if (!clip.isValid()) {
            continue;
        }
        size_t targetLayerIdx = (layerIdx + layerMoved);
        int64_t newClipPosition = clip.position() + frameMoved;

        // range check
        if (targetLayerIdx >= timeline.layersCount()) {
            continue;
        }

        int redius = 3;
        double_t w = clip.duration() * this->zoom;
        double_t x = r.left() + this->frameToX(newClipPosition);
        double_t y = r.top() + this->rowToY(targetLayerIdx);

        QRect ghostRect(x, y + 2.0, w, LAYER_HEIGHT - 4.0);
        QColor bgColor;
        if (drag->isWrong) {
            bgColor = QColor(180, 70, 70, 180);
        } else {
            bgColor = QColor(70, 130, 180, 180);
        };
        QColor strokeColor;
        if (drag->isWrong) {
            strokeColor = QColor(255, 120, 120);
        } else {
            strokeColor = QColor(150, 200, 255);
        };

        p.setBrush(bgColor);
        p.setPen(QPen(strokeColor, 1));
        p.drawRoundedRect(ghostRect, CLIP_ROUND_RADIUS, CLIP_ROUND_RADIUS);
    }
}