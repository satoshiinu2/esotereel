#pragma once

#include <QVBoxLayout>
#include <QWidget>

#include "TimelineCanvasWidget.h"
#include "TimelineToolbarWidget.h"

namespace esotereel::window {
struct WindowGState;

class TimelineWidget : public QWidget {
    Q_OBJECT

  public:
    TimelineCanvasWidget *canvas;
    TimelineToolbarWidget *toolbar;

    explicit TimelineWidget(WindowGState &windowState, size_t timelineIdx) {
        auto *vbox = new QVBoxLayout(this);
        vbox->setContentsMargins(0, 0, 0, 0);
        vbox->setSpacing(0);

        toolbar = new TimelineToolbarWidget(this);
        canvas = new TimelineCanvasWidget(windowState, timelineIdx, this);

        vbox->addWidget(toolbar);
        vbox->addWidget(canvas, 1);

        toolbar->loadButtons(windowState, "timeline", *canvas);
    }
};
} // namespace esotereel::window