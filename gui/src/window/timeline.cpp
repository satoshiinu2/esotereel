
#include "timeline.h"
#include <cstddef>

TimelineWidget::TimelineWidget(size_t timelineType) : timelineType(timelineType), zoom(1.0), scroll(0, 0) {
    hScrollBar = new QScrollBar(Qt::Horizontal, this);
    vScrollBar = new QScrollBar(Qt::Vertical, this);

    connect(hScrollBar, &QScrollBar::valueChanged, [this](int val) {
        this->scroll.setX(val);
        update();
    });
    connect(vScrollBar, &QScrollBar::valueChanged, [this](int val) {
        this->scroll.setY(val);
        update();
    });
}

void TimelineWidget::resizeEvent(QResizeEvent *) {
    int sw = SCROLLBAR_SIZE;
    hScrollBar->setGeometry(0, height() - sw, width() - sw, sw);
    vScrollBar->setGeometry(width() - sw, 0, sw, height() - sw);
}