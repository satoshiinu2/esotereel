#include "timeline.h"
#include <QContextMenuEvent>
#include <QEvent>
#include <QMenu>

void TimelineWidget::contextMenuEvent(QContextMenuEvent *e) {
    QMenu menu(this);

    MTimeline timeline = getTimeline();
    if (timeline.isValid()) {
        auto clipCtx = this->findClipAt(timeline, e->pos());
        if (clipCtx.isValid()) {
            QAction *clipCopyAction = menu.addAction("copy");
            QObject::connect(clipCopyAction, &QAction::triggered, this, []() {});

            QAction *clipDeleteAction = menu.addAction("delete");
            QObject::connect(clipDeleteAction, &QAction::triggered, this, []() {});
        } else {
            QAction *clipAddAction = menu.addAction("Add clip");
            QObject::connect(clipAddAction, &QAction::triggered, this, []() {});
        }
    }

    menu.exec(e->globalPos());
}