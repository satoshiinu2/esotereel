#include "window/widget/timeline/resize.h"
#include "TimelineCanvasWidget.h"
#include "Utils.h"
#include "ffi/project/Clip.h"
#include "ffi/project/CommandQueue.h"
#include "ffi/project/Timeline.h"

using esotereel::contains;

namespace esotereel::window {

// drag.cpp 側の定義(static を外して共有)
std::optional<size_t> rowIndexOfClip(const RenderRows &rows, TimelineId timelineId, uint64_t clipId);

namespace {
struct ClipResizeTarget {
    size_t rowIdx;
    uint64_t layerId;
    int64_t newPosition;
    int64_t newDuration; // 表示用に MIN_CLIP_DURATION 以上へ丸め済み
    bool valid;          // duration >= MIN
};

// clipId の端を frameDelta だけ動かした結果。クリップが見つからなければ nullopt。
std::optional<ClipResizeTarget> computeResizeTarget(const Timeline &timeline, const RenderRows &rows,
                                                    TimelineId timelineId, uint64_t clipId, ClipResizeEdge edge,
                                                    int64_t frameDelta) {
    auto [clip, layerId] = timeline.findClipById(clipId);
    if (!clip.isValid()) {
        return std::nullopt;
    }
    auto rowIdx = rowIndexOfClip(rows, timelineId, clipId);
    if (!rowIdx) {
        return std::nullopt;
    }

    int64_t pos = clip.position();
    int64_t dur = clip.duration();

    if (edge == ClipResizeEdge::Left) {
        pos += frameDelta;
        dur -= frameDelta;
    } else {
        dur += frameDelta;
    }

    bool valid = dur >= MIN_CLIP_DURATION;
    return ClipResizeTarget{*rowIdx, layerId, pos, std::max(dur, MIN_CLIP_DURATION), valid};
}
} // namespace

std::optional<ClipResizeHit> TimelineCanvasWidget::findClipEdgeAt(const QPoint &local) const {
    if (local.x() < LABEL_WIDTH || local.y() < RULER_HEIGHT || !this->cachedRows) {
        return std::nullopt;
    }
    const auto &rows = this->cachedRows->rows();
    int rowIdx = this->YToRow(local.y());
    if (rowIdx < 0 || static_cast<size_t>(rowIdx) >= rows.size()) {
        return std::nullopt;
    }
    const auto &row = rows[rowIdx];
    if (row.timeline_id != this->timelineId || row.node_kind != FfiLayerRowKind::Layer) {
        return std::nullopt;
    }

    for (const auto &c : this->cachedRows->clipsFor(row)) {
        double xs = this->frameToX(c.abs_frame);
        double xe = this->frameToX(c.abs_frame + c.duration);
        if (local.x() < xs || local.x() > xe) {
            continue;
        }
        // 短いクリップでも中央を掴めるよう、ハンドル幅は幅の1/3まで
        double hw = std::min(RESIZE_HANDLE_WIDTH, (xe - xs) / 3.0);
        if (local.x() <= xs + hw) {
            return ClipResizeHit{c.clip_id, ClipResizeEdge::Left};
        }
        if (local.x() >= xe - hw) {
            return ClipResizeHit{c.clip_id, ClipResizeEdge::Right};
        }
    }
    return std::nullopt;
}

std::set<uint64_t> TimelineCanvasWidget::resizeTargetIds(uint64_t grabbedClipId) const {
    if (RESIZE_MULTI_MODE == ResizeMultiMode::AllSelected && contains(this->selectedClipIds, grabbedClipId)) {
        return this->selectedClipIds;
    }
    return {grabbedClipId};
}

bool TimelineCanvasWidget::canResizeClips(const Timeline &timeline, const DragClipResize &drag) const {
    if (!this->cachedRows) {
        return false;
    }
    const int64_t delta = drag.curFrame - drag.srcFrame;
    const auto ids = this->resizeTargetIds(drag.grabbedClipId);

    for (uint64_t clipId : ids) {
        auto t = computeResizeTarget(timeline, *this->cachedRows, this->timelineId, clipId, drag.edge, delta);
        if (!t || !t->valid) {
            return false;
        }
        if (!timeline.canPlaceClipAt(t->layerId, t->newPosition, t->newDuration, ids)) {
            return false;
        }
    }
    return true;
}

std::optional<DragClipResize> TimelineCanvasWidget::handleClipResizeGrab(const QPoint &mousePos, bool ctrl) {
    auto hit = this->findClipEdgeAt(mousePos);
    if (!hit) {
        return std::nullopt;
    }

    if (!esotereel::contains(this->selectedClipIds, hit->clipId)) {
        if (!ctrl) {
            this->selectedClipIds.clear();
        }
        this->selectedClipIds.insert(hit->clipId);
    }
    update();

    int64_t frame = this->XToFrame(mousePos.x());
    return DragClipResize{hit->clipId, hit->edge, frame, frame, false};
}

void TimelineCanvasWidget::handleClipResizeContinue(const Project &project, const QPoint &mousePos) {
    auto *drag = std::get_if<DragClipResize>(&this->dragState);
    if (!drag) {
        return;
    }
    auto timeline = project.timelineOf(this->timelineId);

    drag->curFrame = this->XToFrame(mousePos.x());
    drag->isWrong = !this->canResizeClips(timeline, *drag);
    update();
}

void TimelineCanvasWidget::handleClipResizeDrop(const Project &project, const QPoint &mousePos) {
    this->handleClipResizeContinue(project, mousePos);

    auto *drag = std::get_if<DragClipResize>(&this->dragState);
    if (!drag) {
        return;
    }
    const int64_t delta = drag->curFrame - drag->srcFrame;
    if (drag->isWrong || delta == 0) {
        return;
    }

    const auto idSet = this->resizeTargetIds(drag->grabbedClipId);
    std::vector<uint64_t> ids(idSet.begin(), idSet.end());

    this->commandQueue.resizeClips(project, this->timelineId, ids, drag->edge == ClipResizeEdge::Left, delta);

    this->markRowsDirty();
    update();
}

void TimelineCanvasWidget::drawResizeGhost(const Project &project, QPainter &p, const QRect &r) const {
    auto *drag = std::get_if<DragClipResize>(&this->dragState);
    if (!drag || !this->cachedRows) {
        return;
    }
    auto timeline = project.timelineOf(this->timelineId);
    const int64_t delta = drag->curFrame - drag->srcFrame;

    QColor bgColor = drag->isWrong ? QColor(180, 70, 70, 180) : QColor(70, 130, 180, 180);
    QColor strokeColor = drag->isWrong ? QColor(255, 120, 120) : QColor(150, 200, 255);
    p.setBrush(bgColor);
    p.setPen(QPen(strokeColor, 1));

    for (uint64_t clipId : this->resizeTargetIds(drag->grabbedClipId)) {
        auto t = computeResizeTarget(timeline, *this->cachedRows, this->timelineId, clipId, drag->edge, delta);
        if (!t) {
            continue;
        }
        double_t x = r.left() + this->frameToX(t->newPosition);
        double_t y = r.top() + this->rowToY(t->rowIdx);
        double_t w = t->newDuration * this->zoom;
        QRectF ghostRect(x, y + 2.0, w, LAYER_HEIGHT - 4.0);
        p.drawRoundedRect(ghostRect, CLIP_ROUND_RADIUS, CLIP_ROUND_RADIUS);
    }
}

} // namespace esotereel::window
