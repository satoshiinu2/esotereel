#include "../wrapper/requests.h"
#include "timeline.h"
#include <QContextMenuEvent>
#include <QEvent>
#include <QMenu>
#include <cstddef>
#include <cstdint>
#include <qpoint.h>

void TimelineWidget::contextMenuEvent(QContextMenuEvent *e) {
    QMenu menu(this);

    Timeline timeline = getTimeline();
    if (timeline.isValid()) {
        auto clipCtx = this->findClipAt(timeline, e->pos());
        if (clipCtx.isValid()) {
            QAction *clipCopyAction = menu.addAction("Copy");
            QObject::connect(clipCopyAction, &QAction::triggered, this, []() {});

            QAction *clipDeleteAction = menu.addAction("Delete");
            QObject::connect(clipDeleteAction, &QAction::triggered, this, []() {});
        } else {
            QAction *clipAddAction = menu.addAction("Add clip");
            QObject::connect(clipAddAction, &QAction::triggered, this, [&e, this]() { this->addClipAt(e->pos()); });
        }
    }

    menu.exec(e->globalPos());
}

void TimelineWidget::addClipAt(const QPoint &local) {
    int64_t frame = this->XToFrame(local.x());
    size_t layerIdx = this->YToLayerIdx(local.y());

    Requests::addClipAt(this->timelineIdx, frame, layerIdx);
}