#include "timeline.h"
#include <QEvent>
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <qevent.h>
#include <qpainter.h>
#include <qpair.h>
#include <qwidget.h>

void TimelineWidget::handleCtrlPlayhead(const QPoint &mousePos) {
    if (mousePos.y() > RULER_HEIGHT) {
        return;
    }

    int64_t frame = this->XToFrame(mousePos.x());
    this->playhead = frame;
}

void TimelineWidget::checkEdgeScroll(const QPoint &mousePos, const QRect &r) {
    const qreal edgeZone = 40.0; // エッジから何px以内でスクロールするか
    const qreal maxSpeed = 8.0;

    if (mousePos.x() < edgeZone) {
        qreal speed = (edgeZone - mousePos.x()) / edgeZone * maxSpeed;
        this->setScrollX(std::max(this->scroll.x() - speed, (qreal)0));
    }
    if (mousePos.x() > r.width() - edgeZone) {
        qreal speed = (mousePos.x() - (r.width() - edgeZone)) / edgeZone * maxSpeed;
        this->setScrollX(std::max(this->scroll.x() + speed, (qreal)0));
    }
}

void TimelineWidget::mousePressEvent(QMouseEvent *e) {
    QWidget::mousePressEvent(e);

    if (e->button() & Qt::LeftButton) {
        this->handleCtrlPlayhead(e->pos());
    }
}

void TimelineWidget::mouseMoveEvent(QMouseEvent *e) {
    QWidget::mouseMoveEvent(e);

    if (this->isDragging) {
        this->onDragContinue(e);
    }
    if (e->buttons() & Qt::LeftButton && !this->isDragging) {
        this->onDragStarted(e);
        this->isDragging = true;
    }

    if (this->dragState.has_value()) {
        this->checkEdgeScroll(e->pos(), rect());
    } else if (e->buttons() & Qt::LeftButton) {
        this->handleCtrlPlayhead(e->pos());
    }
}

void TimelineWidget::mouseReleaseEvent(QMouseEvent *e) {
    QWidget::mouseReleaseEvent(e);

    bool ctrl = e->modifiers() & Qt::ControlModifier;

    // ドラッグしていないなら１つセレクト
    if (!this->dragState.has_value() && e->button() & Qt::LeftButton) {
        MTimeline timeline = getTimeline();
        if (timeline.isValid()) {
            this->handleSelectClip(timeline, e->pos(), ctrl);
        }
    }

    if (this->isDragging) {
        this->onDragEnd(e);
        this->isDragging = false;
    }
}

void TimelineWidget::onDragStarted(QMouseEvent *e) {
    bool ctrl = e->modifiers() & Qt::ControlModifier;
    MTimeline timeline = getTimeline();
    if (!timeline.isValid()) {
        goto areasel;
    }

    if (this->handleDragGrab(timeline, e->pos(), ctrl)) {
        return;
    }

areasel:
    this->handleAreaSelStart(e->pos(), ctrl);
}

void TimelineWidget::onDragContinue(QMouseEvent *e) {
    MTimeline timeline = getTimeline();
    if (!timeline.isValid()) {
        goto areasel;
    }

    if (!this->selectionRect.has_value()) {
        this->handleDragContinue(timeline, e->pos());
        return;
    }

areasel:
    this->handleAreaSelContinue(e->pos());
}

void TimelineWidget::onDragEnd(QMouseEvent *e) {
    MTimeline timeline = getTimeline();
    if (!timeline.isValid()) {
        goto areasel;
    }

    if (!this->selectionRect.has_value()) {
        this->handleDragDrop(timeline, e->pos());
        return;
    }

areasel:
    this->handleAreaSelEnd(timeline);
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