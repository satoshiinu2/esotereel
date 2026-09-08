#include "TimelineWidget.h"
#include "Utils.h"
#include "ffi/ClientNetworkHandler.h"
#include "ffi/Requests.h"
#include "ffi/project/Clip.h"
#include "ffi/project/Timeline.h"

namespace esotereel::window {

static std::optional<size_t> rowIndexOfClip(const RenderRows &rows, TimelineId timelineId, uint64_t clipId) {
    size_t rowIdx = 0;
    for (const auto &row : rows.rows()) {
        if (row.timeline_id == timelineId && row.node_kind == FfiLayerRowKind::Layer) {
            for (const auto &clip : rows.clipsFor(row)) {
                if (clip.clip_id == clipId) {
                    return rowIdx;
                }
            }
        }
        rowIdx++;
    }
    return std::nullopt;
}

// ドラッグ移動量(frameMoved/layerMoved)を1クリップに適用した結果の移動先。
struct ClipDropTarget {
    size_t targetLayerIdx;
    uint64_t targetLayerId;
    int64_t newPosition;
    int64_t duration;
};

// clipIdの移動先を計算する。クリップが見つからない場合はnullopt。
std::optional<ClipDropTarget> computeDropTarget(const Timeline &timeline, const RenderRows &rows, TimelineId timelineId,
                                                uint64_t clipId, int64_t frameMoved, int32_t layerMoved) {
    auto clipAndLayer = timeline.findClipById(clipId);
    auto clip = std::get<0>(clipAndLayer);
    if (!clip.isValid()) {
        return std::nullopt;
    }

    auto currentRowIdx = rowIndexOfClip(rows, timelineId, clipId);
    if (!currentRowIdx) {
        return std::nullopt;
    }

    int64_t targetRowIdx = static_cast<int64_t>(*currentRowIdx) + layerMoved;
    if (targetRowIdx < 0 || static_cast<size_t>(targetRowIdx) >= rows.rows().size()) {
        return std::nullopt;
    }

    const auto &targetRow = rows.rows()[static_cast<size_t>(targetRowIdx)];
    if (targetRow.timeline_id != timelineId || targetRow.node_kind != FfiLayerRowKind::Layer) {
        return std::nullopt;
    }

    return ClipDropTarget{
        static_cast<size_t>(targetRowIdx),
        targetRow.node_id,
        clip.position() + frameMoved,
        clip.duration(),
    };
}

// 選択中の全クリップが、指定した移動量の位置に配置可能かどうかを判定する。
// handleClipDragContinue(ドラッグ中の可否表示)とdrawDragGhost(ゴーストの色分け)で共有。
bool TimelineWidget::canDropSelectedClipsAt(const Timeline &timeline, int64_t frameMoved, int32_t layerMoved) const {
    if (!this->cachedRows) {
        return false;
    }
    for (uint64_t clipId : this->selectedClipIds) {
        auto target = computeDropTarget(timeline, *this->cachedRows, this->timelineId, clipId, frameMoved, layerMoved);
        if (!target) {
            continue;
        }
        if (!timeline.canPlaceClipAt(target->targetLayerId, target->newPosition, target->duration,
                                     this->selectedClipIds)) {
            return false;
        }
    }
    return true;
}

std::optional<DragClip> TimelineWidget::handleClipDragGrab(const Project &project, const QPoint &mousePos, bool ctrl) {
    int64_t frame = this->XToFrame(mousePos.x());

    auto clipAndLayer = this->findClipAt(project, mousePos);
    auto clip = std::get<0>(clipAndLayer);
    if (!clip.isValid()) {
        return std::nullopt;
    }

    auto timeline = project.timelineOf(this->timelineId);
    if (!this->cachedRows) {
        return std::nullopt;
    }
    auto layerIdx = rowIndexOfClip(*this->cachedRows, this->timelineId, clip.id());
    if (!layerIdx) {
        return std::nullopt;
    }

    if (!contains(this->selectedClipIds, clip.id())) {
        if (!ctrl) {
            this->selectedClipIds.clear();
        }
        this->selectedClipIds.insert(clip.id());
    }

    update();

    return DragClip{*layerIdx, frame, *layerIdx, frame, mousePos, false};
}

void TimelineWidget::handleClipDragContinue(const Project &project, const QPoint &mousePos) {
    auto timeline = project.timelineOf(this->timelineId);

    auto *drag = std::get_if<DragClip>(&this->dragState);
    if (!drag) {
        return;
    }

    drag->curFrame = this->XToFrame(mousePos.x());
    drag->curLayerIdx = this->YToRow(mousePos.y());
    drag->ghostPos = mousePos;

    int64_t frameMoved = drag->curFrame - drag->srcFrame;
    int32_t layerMoved = drag->curLayerIdx - drag->srcLayerIdx;

    drag->isWrong = !canDropSelectedClipsAt(timeline, frameMoved, layerMoved);

    update();
}

void TimelineWidget::handleClipDraggingDrop(const Project &project, const QPoint &mousePos) {
    auto timeline = project.timelineOf(this->timelineId);

    handleClipDragContinue(project, mousePos);

    auto *drag = std::get_if<DragClip>(&this->dragState);
    if (!drag) {
        return;
    }

    int64_t frameMoved = drag->curFrame - drag->srcFrame;
    int32_t layerMoved = drag->curLayerIdx - drag->srcLayerIdx;

    // NOTE: 元実装では配置可否をここでも再チェックしていましたが、結果に関わらず
    // 常に同じ場所へ処理が合流し moveClips を送信していたため、判定自体は
    // ドロップの可否に影響していませんでした(handleClipDragContinue内の
    // drag->isWrong 表示のみが実質的な可否フィードバックです)。
    // 挙動は変えず、意味の無い重複ループのみ削除しています。
    std::vector<uint64_t> exclude_vec(this->selectedClipIds.begin(), this->selectedClipIds.end());

    this->windowState.network->requests().moveClips(this->timelineId, exclude_vec, frameMoved, 0, layerMoved);

    this->markRowsDirty();
    update();
}

void TimelineWidget::drawDragGhost(const Project &project, QPainter &p, const QRect &r) const {
    auto timeline = project.timelineOf(this->timelineId);

    auto *drag = std::get_if<DragClip>(&this->dragState);
    if (!drag) {
        return;
    }

    int64_t frameMoved = drag->curFrame - drag->srcFrame;
    int32_t layerMoved = drag->curLayerIdx - drag->srcLayerIdx;

    QColor bgColor = drag->isWrong ? QColor(180, 70, 70, 180) : QColor(70, 130, 180, 180);
    QColor strokeColor = drag->isWrong ? QColor(255, 120, 120) : QColor(150, 200, 255);

    for (uint64_t clipId : this->selectedClipIds) {
        if (!this->cachedRows) {
            continue;
        }
        auto target = computeDropTarget(timeline, *this->cachedRows, this->timelineId, clipId, frameMoved, layerMoved);
        if (!target) {
            continue;
        }
        double_t w = target->duration * this->zoom;
        double_t x = r.left() + this->frameToX(target->newPosition);
        double_t y = r.top() + this->rowToY(target->targetLayerIdx);

        QRect ghostRect(x, y + 2.0, w, LAYER_HEIGHT - 4.0);

        p.setBrush(bgColor);
        p.setPen(QPen(strokeColor, 1));
        p.drawRoundedRect(ghostRect, CLIP_ROUND_RADIUS, CLIP_ROUND_RADIUS);
    }
}
} // namespace esotereel::window
