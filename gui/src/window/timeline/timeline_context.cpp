#include "../../wrapper/network.h"
#include "../../wrapper/project/clip.h"            // IWYU pragma: keep
#include "../../wrapper/project/layer.h"           // IWYU pragma: keep
#include "../../wrapper/project/layer_clips.h"     // IWYU pragma: keep
#include "../../wrapper/project/project.h"         // IWYU pragma: keep
#include "../../wrapper/project/timeline.h"        // IWYU pragma: keep
#include "../../wrapper/project/timeline_layers.h" // IWYU pragma: keep
#include "../../wrapper/requests.h"
#include "../main.h"
#include "timeline.h"
#include <QContextMenuEvent>
#include <QEvent>
#include <QMenu>
#include <QPoint>
#include <cstddef>
#include <cstdint>

void TimelineWidget::contextMenuEvent(QContextMenuEvent *e) {
    QMenu menu(this);

    auto project = windowState.network->getProject();
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
    size_t layerOrder = this->YToRow(local.y());

    windowState.network->requests().addClipAt(this->timelineIdx, frame, layerOrder);
}