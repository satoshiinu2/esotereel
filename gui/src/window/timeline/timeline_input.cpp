#include "../../wrapper/network.h"
#include "../../wrapper/project/project.h"
#include "../../wrapper/project/timeline.h"
#include "../main.h"
#include "esotereel_gui_helper.h"
#include "timeline.h"
#include <QDebug>
#include <QEvent>
#include <QNativeGestureEvent>
#include <QPainter>
#include <QPair>
#include <QPoint>
#include <QWidget>
#include <algorithm>
#include <cmath>
#include <cstdint>

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
        // クリック時にこのウィジェットにフォーカスを移す
        this->setFocus();
        // update focused widget
        this->windowState.focusedTimeline = this;

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

    Project project = windowState.network->getProject();
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
        update();
    }
}

DragState TimelineWidget::onDragStarted(QMouseEvent *e, QPoint firstClickPos) {
    bool ctrl = e->modifiers() & Qt::ControlModifier;

    Project project = windowState.network->getProject();
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
    Project project = windowState.network->getProject();
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
    Project project = windowState.network->getProject();
    Timeline timeline = getTimeline(project);

    if (!timeline.isValid()) {
        return;
    }

    std::visit(
        [&](auto &&state) {
            using T = std::decay_t<decltype(state)>;

            if constexpr (std::is_same_v<T, DragAreaSel>) {
                this->handleAreaSelEnd(project, timeline);
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

bool TimelineWidget::event(QEvent *e) {
    if (e->type() == QEvent::NativeGesture) {
        auto *ge = static_cast<QNativeGestureEvent *>(e);
        if (ge->gestureType() == Qt::ZoomNativeGesture) {
            // ノートパソコンのピンチ操作（ズームジェスチャー）
            // プラットフォームによって ge->value() の意味が異なります：
            // - macOS: 1.0 を基準とした倍率（1.1 なら 110%）
            // - Windows: 0.0 を基準とした変化量（0.00390625 なら +0.39%）
            qreal value = ge->value();

            // 0付近の値（変化量）が送られてきた場合は 1.0 を足して倍率に変換する
            qreal factor = (std::abs(value) < 0.5) ? (1.0 + value) : value;

            if (factor > 0) {
                this->zoom = std::clamp(this->zoom * (float_t)factor, 0.1f, 10.0f);
                update();
                return true;
            }
        }
    }
    return QWidget::event(e);
}

void TimelineWidget::mouseDoubleClickEvent(QMouseEvent *e) {
    QWidget::mouseDoubleClickEvent(e);
}

void TimelineWidget::keyPressEvent(QKeyEvent *e) {
    if (e->key() == Qt::Key_Space) {
        this->togglePlayback();
        e->accept();
    } else {
        QWidget::keyPressEvent(e);
    }
}