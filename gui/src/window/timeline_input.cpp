#include "timeline.h"
#include <QEvent>
#include <qevent.h>

void TimelineWidget::mousePressEvent(QMouseEvent *e) {}

void TimelineWidget::mouseReleaseEvent(QMouseEvent *e) {}

void TimelineWidget::mouseMoveEvent(QMouseEvent *e) {}

void TimelineWidget::wheelEvent(QWheelEvent *e) {
    QPoint delta = e->angleDelta(); // 1ノッチ = 120

    if (e->modifiers() & Qt::ControlModifier) {
        // Ctrl+ホイール → ズーム
        float_t factor = delta.y() > 0 ? 1.1f : 0.9f;
        this->zoom = std::clamp(this->zoom * factor, 0.1f, 10.0f);
    } else {
        // 横スクロール
        this->scroll.setX(std::max(0.0f, (float_t)this->scroll.x() - delta.x()));
        hScrollBar->setValue(this->scroll.x());
        // 縦スクロール
        this->scroll.setY(std::max(0.0f, (float_t)this->scroll.y() - delta.y()));
        vScrollBar->setValue(this->scroll.y());
    }

    update();
    e->accept();
}

void TimelineWidget::mouseDoubleClickEvent(QMouseEvent *e) {}