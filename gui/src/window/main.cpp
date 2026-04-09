#include "main.h"
#include "timeline.h"
#include "wgpu_canvas.h"
#include <DockManager.h>
#include <QLabel>

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
    resize(1280, 720);
    setWindowTitle("Esotereel");

    auto *central = new QLabel("Preview", this);
    central->setAlignment(Qt::AlignCenter);
    setCentralWidget(central);

    this->dockManager = new ads::CDockManager(this);
    ads::CDockManager::setConfigFlag(ads::CDockManager::AlwaysShowTabs, false);

    auto *previewWidget = new WgpuCanvasWidget();
    auto *previewDock = new ads::CDockWidget(this->dockManager, "Preview");
    previewDock->setWidget(previewWidget);
    dockManager->addDockWidget(ads::BottomDockWidgetArea, previewDock);

    this->timelineWidget = new TimelineWidget(0);
    auto *timelineDock = new ads::CDockWidget(this->dockManager, "Timeline");
    timelineDock->setWidget(timelineWidget);
    dockManager->addDockWidget(ads::BottomDockWidgetArea, timelineDock);
}