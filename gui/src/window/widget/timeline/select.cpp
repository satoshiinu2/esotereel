#include "TimelineCanvasWidget.h"
#include "window/widget/timeline/select.h"

using ClipId = esotereel_gui_helper::ClipId;

namespace esotereel::window {
// clipSelectionChanged を次のイベントループ周回へ遅延して emit する。
// 呼び出し元 (handleSelectClip / handleAreaSelEnd) は Project の read
// ガードを保持したまま呼ばれる。接続先の ClipPropertiesPanel::setClips
// -> rebuild() は getProject() を呼ぶため、ここで直接 emit すると
// ガード保持中のまま同一 RwLock の read を入れ子に取得してしまう。
// 選択内容は値でキャプチャするので、遅延実行時点の状態には依存しない。
void TimelineCanvasWidget::emitClipSelectionChangedDeferred() {
    std::vector<ClipId> clipIds(this->selectedClipIds.begin(), this->selectedClipIds.end());
    TimelineId tid = this->timelineId;
    QTimer::singleShot(0, this, [this, tid, clipIds]() { emit clipSelectionChanged(tid, clipIds); });
}

// return true if selected
bool TimelineCanvasWidget::handleSelectClip(const Project &project, const QPoint &mousePos, bool ctrl) {
    auto [clip, layerId] = this->findClipAt(project, mousePos);
    if (!clip.isValid()) {
        if (!ctrl) {
            this->selectedClipIds.clear();
            this->emitClipSelectionChangedDeferred();
            update();
            return false;
        }
        return false;
    }

    ClipId id = clip.id();
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
    this->emitClipSelectionChangedDeferred();
    update();
    return true;
}

std::optional<DragAreaSel> TimelineCanvasWidget::handleAreaSelStart(const QPoint &mousePos, bool ctrl) {
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

void TimelineCanvasWidget::handleAreaSelContinue(const QPoint &mousePos) {
    if (auto *sel = std::get_if<DragAreaSel>(&this->dragState)) {
        sel->current = mousePos;
    }
    update();
}

void TimelineCanvasWidget::handleAreaSelEnd(const Project &project) {
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

    this->emitClipSelectionChangedDeferred();
    update();
}

void TimelineCanvasWidget::drawSelectionRect(QPainter &p, const QRect &r) const {
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