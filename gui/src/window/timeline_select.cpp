#include "timeline.h"
#include <QEvent>
#include <cmath>
#include <cstdint>
#include <optional>
#include <qpainter.h>
#include <qpoint.h>

// return true if selected
bool TimelineWidget::handleSelectClip(MTimeline &timeline, const QPoint &mousePos, bool ctrl) {
    auto clipLoc = this->findClipAt(timeline, mousePos);
    if (!clipLoc.isValid()) {
        if (!ctrl) {
            this->selectedClipIds.clear();
            update();
            return false;
        }
        return false;
    }

    uint64_t id = clipLoc.clip.id();
    if (ctrl) {
        if (this->selectedClipIds.count(id)) {
            this->selectedClipIds.erase(id);
        } else {
            this->selectedClipIds.insert(id);
        }
    } else {
        this->selectedClipIds.clear();
        this->selectedClipIds.insert(id);
    }
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
            double_t clipXStart = this->frameToX(clip.position());
            double_t clipXEnd = this->frameToX(clip.position() + clip.duration());
            double_t clipYStart = this->layerToY(layerIdx);
            double_t clipYEnd = clipYStart + LAYER_HEIGHT;

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