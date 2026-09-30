#pragma once

#include <QVBoxLayout>
#include <QWidget>

#include "TimelineCanvasWidget.h"
#include "TimelineToolbarWidget.h"

namespace esotereel::window {
struct WindowGState;

using TimelineId = esotereel_gui_helper::TimelineId;

class TimelineWidget : public QWidget {
    Q_OBJECT

  public:
    TimelineCanvasWidget *canvas;
    TimelineToolbarWidget *toolbar;

    explicit TimelineWidget(WindowGState &windowState, TimelineId timelineId) {
        auto *vbox = new QVBoxLayout(this);
        vbox->setContentsMargins(0, 0, 0, 0);
        vbox->setSpacing(0);

        toolbar = new TimelineToolbarWidget(timelineId, this);
        canvas = new TimelineCanvasWidget(windowState, timelineId, this);

        vbox->addWidget(toolbar);
        vbox->addWidget(canvas, 1);

        toolbar->loadButtons(windowState, "timeline", *canvas);
    }
};
} // namespace esotereel::window