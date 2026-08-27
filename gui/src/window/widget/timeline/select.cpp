#include "TimelineWidget.h"
#include "wrapper/project/Timeline.h"

namespace esotereel::window {
// return true if selected
bool TimelineWidget::handleSelectClip(const Project &project, const QPoint &mousePos, bool ctrl) {
    auto [clip, layerId] = this->findClipAt(project, mousePos);
    if (!clip.isValid()) {
        if (!ctrl) {
            this->selectedClipIds.clear();
            update();
            return false;
        }
        return false;
    }

    uint64_t id = clip.id();
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

std::optional<DragAreaSel> TimelineWidget::handleAreaSelStart(const QPoint &mousePos, bool ctrl) {
    if (mousePos.x() <= LABEL_WIDTH || mousePos.y() <= RULER_HEIGHT) {
        return std::nullopt;
    }

    if (!ctrl) {
        this->selectedClipIds.clear();
    }

    update();
    return DragAreaSel{
        mousePos,
        mousePos,
    };
}

void TimelineWidget::handleAreaSelContinue(const QPoint &mousePos) {
    if (auto *sel = std::get_if<DragAreaSel>(&this->dragState)) {
        sel->current = mousePos;
    }
    update();
}

void TimelineWidget::handleAreaSelEnd(const Project &project) {
    auto *sel = std::get_if<DragAreaSel>(&this->dragState);
    if (!sel) {
        return;
    }

    QRect selRect = QRect(sel->start.toPoint(), sel->current.toPoint()).normalized();

    std::unique_ptr<RenderRows> &rr = this->cachedRows;
    if (!rr) {
        return;
    }

    // TODO: optimize
    size_t layerIdx = 0;
    for (auto const &row : rr->rows()) {
        for (auto const &info : rr->clipsFor(row)) {
            double_t clipXStart = this->frameToX(info.abs_frame);
            double_t clipXEnd = this->frameToX(info.abs_frame + info.duration);
            double_t clipYStart = this->rowToY(layerIdx);
            double_t clipYEnd = clipYStart + LAYER_HEIGHT;

            QRect clip_rect(clipXStart, clipYStart, clipXEnd, clipYEnd);

            if (selRect.intersects(clip_rect)) {
                this->selectedClipIds.insert(info.clip_id);
            }
        }
        layerIdx++;
    }

    update();
}

void TimelineWidget::drawSelectionRect(QPainter &p, const QRect &r) const {
    auto *sel = std::get_if<DragAreaSel>(&this->dragState);
    if (!sel) {
        return;
    }

    QRectF selRect(r.topLeft() + sel->start, r.topLeft() + sel->current);

    p.setBrush(QColor(100, 150, 255, 64));
    p.setPen(QPen(QColor(100, 150, 255), 1));
    p.drawRect(selRect);
}
} // namespace esotereel::window