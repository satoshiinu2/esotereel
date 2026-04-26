
#include "timeline.h"
#include <cmath>
#include <cstddef>
#include <qpoint.h>
#include <qwidget.h>
#include <tuple>

TimelineWidget::TimelineWidget(WindowGState *windowState, size_t timelineType) : windowState(windowState), timelineIdx(timelineType) {
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

std::tuple<Clip, size_t> TimelineWidget::findClipAt(const Timeline &timeline, const QPoint &local) const {
    int64_t frame = this->XToFrame(local.x());
    size_t layerIdx = this->YToLayerIdx(local.y());

    // range check
    if (layerIdx >= timeline.layersCount()) {
        return std::make_tuple(Clip::Empty(), 0);
    }
    Clip clip = timeline.layerAt(layerIdx).findClipAtFrame(frame);
    return std::make_tuple(clip, layerIdx);
}
