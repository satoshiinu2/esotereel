#include "main.h"
#include "timeline.h"
#include <DockManager.h>
#include <QLabel>

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
    resize(1280, 720);
    setWindowTitle("MusclEdit");

    auto *central = new QLabel("Preview", this);
    central->setAlignment(Qt::AlignCenter);
    setCentralWidget(central);

    this->dockManager = new ads::CDockManager(this);

    auto *previewWidget = new QWidget();
    auto *previewDock = new ads::CDockWidget("Preview");
    previewDock->setWidget(previewWidget);
    dockManager->addDockWidget(ads::BottomDockWidgetArea, previewDock);

    auto *timelineWidget = new TimelineWidget(0);
    auto *timelineDock = new ads::CDockWidget("Timeline");
    timelineDock->setWidget(timelineWidget);
    dockManager->addDockWidget(ads::BottomDockWidgetArea, timelineDock);
}