
#include "timeline.h"
#include <cmath>
#include <cstddef>
#include <qpoint.h>
#include <qwidget.h>

TimelineWidget::TimelineWidget(size_t timelineType) : timelineType(timelineType) {
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

void TimelineWidget::resizeEvent(QResizeEvent *event) {
    QWidget::resizeEvent(event);

    int sw = SCROLLBAR_SIZE;
    hScrollBar->setGeometry(0, height() - sw, width() - sw, sw);
    vScrollBar->setGeometry(width() - sw, 0, sw, height() - sw);
}

MClipLocation TimelineWidget::findClipAt(const MTimeline &timeline, const QPoint &local) const {
    int64_t frame = this->XToFrame(local.x());
    size_t layerIdx = ((local.y() - RULER_HEIGHT + this->scroll.y()) / LAYER_HEIGHT);

    // range check
    if (layerIdx >= timeline.layersCount()) {
        return MClipLocation();
    }
    return timeline.layerAt(layerIdx).findClipAtFrame(frame, layerIdx);
}
