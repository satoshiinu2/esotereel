#include "../util.h"
#include "timeline.h"
#include <QEvent>
#include <cmath>
#include <optional>
#include <qpainter.h>
#include <qpoint.h>

bool TimelineWidget::handleSelectClip(MTimeline &timeline, const QPoint &mousePos, bool ctrl) {
    auto clipLoc = this->findClipAt(timeline, mousePos);
    if (!clipLoc.isValid()) {
        this->selectedClipIds.clear();
        return false;
    };

    if (!ctrl) {
        this->selectedClipIds.clear();
    }
    this->selectedClipIds.insert(clipLoc.clip.id());
    update();
    return true;
}

void TimelineWidget::handleAreaSelStart(const QPoint &mousePos, bool ctrl) {
    if (!ctrl) {
        this->selectedClipIds.clear();
    }
    this->selectionRect = SelectionRect{
        mousePos,
        mousePos,
    };
    update();
}

void TimelineWidget::handleAreaSelContinue(const QPoint &mousePos) {
    if (this->selectionRect.has_value()) {
        this->selectionRect->current = mousePos;
    }
    update();
}

void TimelineWidget::handleAreaSelEnd(const MTimeline &timeline) {
    auto sel = this->selectionRect;
    if (!sel.has_value()) {
        return;
    }

    QRect selRect(sel->start.toPoint(), sel->current.toPoint());

    size_t layerIdx = 0;
    for (auto layer : timeline.layers()) {
        size_t clipIdx = 0;
        for (auto clip : layer.clips()) {
            float_t clipXStart = this->frameToX(clip.position());
            float_t clipXEnd = this->frameToX(clip.position() + clip.duration());
            float_t clipYStart = this->layerToY(layerIdx);
            float_t clipYEnd = clipYStart + LAYER_HEIGHT;

            QRect clip_rect(clipXStart, clipYStart, clipXEnd, clipYEnd);

            if (selRect.intersects(clip_rect)) {
                this->selectedClipIds.insert(clip.id());
            }
            clipIdx++;
        }
        layerIdx++;
    }

    this->selectionRect = std::nullopt;
    update();
}

void TimelineWidget::drawSelectionRect(QPainter &p, const QRect &r) const {
    if (!this->selectionRect.has_value()) {
        return;
    }
    auto sel = selectionRect.value();

    QRectF selRect(r.topLeft() + sel.start, r.topLeft() + sel.current);

    p.setBrush(QColor(100, 150, 255, 64));
    p.setPen(QPen(QColor(100, 150, 255), 1));
    p.drawRect(selRect);
}