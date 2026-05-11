#include "debug_streams.h"

#include <QPainter>

DebugStreamsWidget::DebugStreamsWidget(QWidget *parent) : QWidget(parent), hScrollBar(nullptr) {
    // フローティング時に消えないよう最小サイズを設定
    setMinimumSize(320, 240);
}

void DebugStreamsWidget::paintEvent(QPaintEvent *e) {
    QWidget::paintEvent(e);

    QPainter p(this);
    QRect r = rect();
    // 背景
    p.fillRect(r, palette().window()); // システムの背景色を使用
}
