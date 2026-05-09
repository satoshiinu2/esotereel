#include "../wrapper/requests.h"
#include "main.h"
#include "timeline.h"
#include <QContextMenuEvent>
#include <QEvent>
#include <QMenu>
#include <cstddef>
#include <cstdint>
#include <qpoint.h>

void TimelineWidget::contextMenuEvent(QContextMenuEvent *e) {
    QMenu menu(this);

    auto project = windowState->network->getProject();
    Timeline timeline = getTimeline(project);
    if (timeline.isValid()) {
        auto [clip, layerIdx] = this->findClipAt(timeline, e->pos());
        if (clip.isValid()) {
            QAction *clipCopyAction = menu.addAction("Copy");
            QObject::connect(clipCopyAction, &QAction::triggered, this, []() {});

            QAction *clipDeleteAction = menu.addAction("Delete");
            QObject::connect(clipDeleteAction, &QAction::triggered, this, []() {});
        } else {
            QPoint pos = e->pos();
            QAction *clipAddAction = menu.addAction("Add clip");
            QObject::connect(clipAddAction, &QAction::triggered, this, [pos, this]() { this->addClipAt(pos); });

            QAction *clipTestAction = menu.addAction("request test");
            QObject::connect(clipTestAction, &QAction::triggered, this, [&project]() { project.debugLog(); });
        }
    }

    menu.exec(e->globalPos());
}

void TimelineWidget::addClipAt(const QPoint &local) {
    int64_t frame = this->XToFrame(local.x());
    size_t layerIdx = this->YToLayerIdx(local.y());

    windowState->network->requests().addClipAt(this->timelineIdx, frame, layerIdx);
}