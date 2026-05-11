
#include "../wrapper/network.h"
#include "../wrapper/project/project.h"
#include "../wrapper/project/timeline.h"
#include "main.h"
#include "timeline.h"
#include <QEvent>
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <qevent.h>
#include <qlogging.h>
#include <qpainter.h>
#include <qpair.h>
#include <qpoint.h>
#include <qwidget.h>

void TimelineWidget::handleCtrlPlayhead(const QPoint &mousePos) {
    double_t mouseX = mousePos.x();
    mouseX = std::max((double_t)LABEL_WIDTH, mouseX);

    int64_t frame = this->XToFrame(mouseX);
    this->playhead = frame;
    update();
}

bool isOnRuler(QPoint &mousePos) {
    return mousePos.y() <= RULER_HEIGHT;
}

void TimelineWidget::checkEdgeScroll(const QPoint &mousePos, const QRect &r) {
    const qreal edgeZone = 40.0; // エッジから何px以内でスクロールするか
    const qreal maxSpeed = 8.0;

    qreal oldX = this->scroll.x();
    qreal oldY = this->scroll.y();

    if (mousePos.x() < edgeZone) {
        qreal speed = (edgeZone - mousePos.x()) / edgeZone * maxSpeed;
        this->setScrollX(std::max(this->scroll.x() - speed, (qreal)0));
    }
    if (mousePos.x() > r.width() - edgeZone) {
        qreal speed = (mousePos.x() - (r.width() - edgeZone)) / edgeZone * maxSpeed;
        this->setScrollX(std::max(this->scroll.x() + speed, (qreal)0));
    }

    // 再描画
    if (this->scroll.x() != oldX || this->scroll.y() != oldY) {
        update();
        hScrollBar->setValue(this->scroll.x());
        vScrollBar->setValue(this->scroll.y());
    }
}

void TimelineWidget::mousePressEvent(QMouseEvent *e) {
    QWidget::mousePressEvent(e);
    auto mousePos = e->pos();

    if (e->button() & Qt::LeftButton) {
        // update focused widget
        this->windowState->focusedTimeline = this;

        if (isOnRuler(mousePos)) {
            this->handleCtrlPlayhead(mousePos);
        }
        this->firstClickPos = mousePos;
        this->dragState = DragNone{};
    }
}

void TimelineWidget::mouseMoveEvent(QMouseEvent *e) {
    QWidget::mouseMoveEvent(e);

    if (e->buttons() & Qt::LeftButton && std::holds_alternative<DragNone>(this->dragState) &&
        this->firstClickPos.has_value()) {
        this->dragState = this->onDragStarted(e, this->firstClickPos.value());
    }

    if (!std::holds_alternative<DragNone>(this->dragState)) {
        this->onDragContinue(e);
    }
}

void TimelineWidget::mouseReleaseEvent(QMouseEvent *e) {
    QWidget::mouseReleaseEvent(e);

    Project project = windowState->network->getProject();
    Timeline timeline = getTimeline(project);

    bool ctrl = e->modifiers() & Qt::ControlModifier;

    // ドラッグしていないなら１つセレクト
    if (!std::holds_alternative<DragClip>(this->dragState) && e->button() & Qt::LeftButton) {

        if (timeline.isValid()) {
            this->handleSelectClip(timeline, e->pos(), ctrl);
        }
    }

    if (e->button() & Qt::LeftButton && !std::holds_alternative<DragNone>(this->dragState)) {
        this->onDragEnd(e);
        this->dragState = DragNone{};
    }
}

DragState TimelineWidget::onDragStarted(QMouseEvent *e, QPoint firstClickPos) {
    bool ctrl = e->modifiers() & Qt::ControlModifier;

    Project project = windowState->network->getProject();
    Timeline timeline = getTimeline(project);

    if (!timeline.isValid()) {
        return DragOther{};
    }

    // 1. クリップを掴めるかチェック
    auto clipGrabResult = this->handleClipDragGrab(timeline, firstClickPos, ctrl);
    if (clipGrabResult.has_value()) {
        return clipGrabResult.value();
    }

    // 2. ルーラー上なら再生ヘッド移動
    if (isOnRuler(firstClickPos)) {
        return DragPlayHead{};
    }

    // 3. 何もなければ範囲選択を開始
    auto areaSelResult = this->handleAreaSelStart(e->pos(), ctrl);
    if (areaSelResult.has_value()) {
        return areaSelResult.value();
    }

    return DragOther{};
}

void TimelineWidget::onDragContinue(QMouseEvent *e) {
    Project project = windowState->network->getProject();
    Timeline timeline = getTimeline(project);

    std::visit(
        [&](auto &&state) {
            using T = std::decay_t<decltype(state)>;

            // タイムラインないならセレクトだけする
            if (!timeline.isValid()) {
                this->handleAreaSelContinue(e->pos());
            } else if constexpr (std::is_same_v<T, DragAreaSel>) {
                this->handleAreaSelContinue(e->pos());
            } else if constexpr (std::is_same_v<T, DragPlayHead>) {
                this->handleCtrlPlayhead(e->pos());
            } else if constexpr (std::is_same_v<T, DragClip>) {
                this->handleClipDragContinue(timeline, e->pos());
                this->checkEdgeScroll(e->pos(), rect());
            }
        },
        this->dragState);
}

void TimelineWidget::onDragEnd(QMouseEvent *e) {
    auto project = windowState->network->getProject();
    Timeline timeline = getTimeline(project);

    if (!timeline.isValid()) {
        return;
    }

    std::visit(
        [&](auto &&state) {
            using T = std::decay_t<decltype(state)>;

            if constexpr (std::is_same_v<T, DragAreaSel>) {
                this->handleAreaSelEnd(timeline);
            } else if constexpr (std::is_same_v<T, DragPlayHead>) {

            } else if constexpr (std::is_same_v<T, DragClip>) {
                this->handleClipDraggingDrop(timeline, e->pos());
            }
        },
        this->dragState);
}

void TimelineWidget::wheelEvent(QWheelEvent *e) {
    QWidget::wheelEvent(e);

    QPoint delta = e->angleDelta(); // 1ノッチ = 120
    bool ctrl = e->modifiers() & Qt::ControlModifier;

    if (ctrl) {
        // Ctrl+ホイール → ズーム
        float_t factor = delta.y() > 0 ? 1.1f : 0.9f;
        this->zoom = std::clamp(this->zoom * factor, 0.1f, 10.0f);
    } else {
        // スクロール
        this->scroll.setX(std::max(0.0f, (float_t)this->scroll.x() - delta.x()));
        this->scroll.setY(std::max(0.0f, (float_t)this->scroll.y() - delta.y()));

        hScrollBar->setValue(this->scroll.x());
        vScrollBar->setValue(this->scroll.y());
    }

    update();
    e->accept();
}

void TimelineWidget::mouseDoubleClickEvent(QMouseEvent *e) {
    QWidget::mouseDoubleClickEvent(e);
}