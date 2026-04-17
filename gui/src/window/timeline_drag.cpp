#include "../util.h"
#include "../wrapper/requests.h"
#include "timeline.h"
#include <QEvent>
#include <cmath>
#include <cstdint>
#include <optional>
#include <qcolor.h>
#include <qevent.h>
#include <qpainter.h>

bool TimelineWidget::handleDragGrab(const Timeline &timeline, const QPoint &mousePos, bool ctrl) {
    uint64_t frame = this->XToFrame(mousePos.x());

    auto clipCtx = this->findClipAt(timeline, mousePos);
    if (!clipCtx.isValid()) {
        return false;
    }
    const Clip *clip = &clipCtx.clip;

    if (!contains(this->selectedClipIds, clip->id())) {
        if (!ctrl) {
            this->selectedClipIds.clear();
        }
        this->selectedClipIds.insert(clip->id());
    }

    this->dragState = ClipDragState{
        clipCtx.layerIdx,
        frame,
        clipCtx.layerIdx,
        frame,
        mousePos,
        false};

    update();
    return true;
}

void TimelineWidget::handleDragContinue(const Timeline &timeline, const QPoint &mousePos) {
    int64_t frame = this->XToFrame(mousePos.x());
    ssize_t layerIdx = this->YToLayerIdx(mousePos.y());

    auto &drag = this->dragState;
    if (!drag.has_value()) {
        return;
    }

    drag->curFrame = frame;
    drag->curLayerIdx = layerIdx;
    drag->ghostPos = mousePos;

    // ovetlap check
    int frameMoved = drag->curFrame - drag->srcFrame;
    int layerMoved = drag->curLayerIdx - drag->srcLayerIdx;

    drag->isWrong = false;
    for (uint64_t clipid : this->selectedClipIds) {
        auto clipLoc = timeline.findClipById(clipid);
        if (!clipLoc.isValid()) {
            continue;
        }
        size_t targetLayerIdx = (clipLoc.layerIdx + layerMoved);
        int64_t newClipPosition = clipLoc.clip.position() + frameMoved;
        if (!timeline.canPlaceClipAt(targetLayerIdx, newClipPosition,
                                     clipLoc.clip.duration(), this->selectedClipIds)) {
            this->dragState->isWrong = true;
            goto finalize;
        }
    }

finalize:
    update();
}

void TimelineWidget::handleDragDrop(const Timeline &timeline, const QPoint &mousePos) {
    handleDragContinue(timeline, mousePos);
    auto &drag = this->dragState;
    if (!drag.has_value()) {
        return;
    }

    uint64_t frameMoved = drag->curFrame - drag->srcFrame;
    int layerMoved = drag->curLayerIdx - drag->srcLayerIdx;
    // range and overrap check
    for (uint64_t clipId : this->selectedClipIds) {
        auto clipLoc = timeline.findClipById(clipId);
        if (!clipLoc.isValid()) {
            continue;
        }
        size_t targetLayerIdx = (clipLoc.layerIdx + layerMoved);
        int64_t newClipPosition = clipLoc.clip.position() + frameMoved;
        if (!timeline.canPlaceClipAt(targetLayerIdx, newClipPosition,
                                     clipLoc.clip.duration(), this->selectedClipIds)) {
            goto send_drop;
        }
    }

send_drop:
    update();
    this->dragState = std::nullopt;

    std::vector<uint64_t>
        exclude_vec(this->selectedClipIds.begin(), this->selectedClipIds.end());
    Requests::moveClips(this->timelineIdx, exclude_vec, frameMoved, 0, layerMoved);
}

void TimelineWidget::drawDragGhost(const Timeline &timeline, QPainter &p, const QRect &r) const {
    auto drag = this->dragState;
    if (!drag.has_value()) {
        return;
    }

    int64_t frameMoved = drag->curFrame - drag->srcFrame;
    int64_t layerMoved = drag->curLayerIdx - drag->srcLayerIdx;
    // range and overrap check
    for (uint64_t clipId : this->selectedClipIds) {
        auto clipLoc = timeline.findClipById(clipId);
        if (!clipLoc.isValid()) {
            continue;
        }
        size_t targetLayerIdx = (clipLoc.layerIdx + layerMoved);
        int64_t newClipPosition = clipLoc.clip.position() + frameMoved;

        // range check
        if (targetLayerIdx >= timeline.layersCount()) {
            continue;
        }

        int redius = 3;
        double_t w = clipLoc.clip.duration() * this->zoom;
        double_t x = r.left() + this->frameToX(newClipPosition);
        double_t y = r.top() + this->layerToY(targetLayerIdx);

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