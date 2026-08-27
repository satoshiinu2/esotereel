#include "MainWindow.h"
#include "../wrapper/project/camera.h"
#include "ads_globals.h"
#include "widget/preview/GpuPreviewWidget.h"
#include "widget/timeline/TimelineWidget.h"
#include <DockManager.h>
#include <QLabel>
#include <QMenuBar>
#include <QTimer>
#include <QVector3D>
#include <QWidget>

#define SHOW_DEBUG_STREAMS 1

#if SHOW_DEBUG_STREAMS
#include "widget/debug_streams/DebugStreamsWidget.h"
#endif

namespace esotereel::window {

MainWindow::MainWindow(ClientNetworkHandler &network, QWidget *parent) : QMainWindow(parent) {
    this->windowState.network = &network;

    this->windowState.camera = new CameraInfo{};
    this->windowState.camera->position = QVector3D(0, 0, 0);
    this->windowState.camera->rotation = QVector3D(0, 0, 0);
    this->windowState.camera->is_orthographic = true;
    this->windowState.camera->orthographic_direction = Direction::Front;
    this->windowState.camera->scale_factor = 1.0;
    this->windowState.camera->fov = 60.0;

    resize(1280, 720);
    setWindowTitle("Esotereel");

    ads::CDockManager::setConfigFlag(ads::CDockManager::AlwaysShowTabs, false);
    this->dockManager = new ads::CDockManager(this);
    setCentralWidget(this->dockManager);

    // タブの文字色をスタイルシートで設定
    this->dockManager->setStyleSheet(
        "ads--CDockWidgetTab { color: palette(mid); font-size: 11px; }" // 非アクティブ（中間色）
        "ads--CDockWidgetTab[activeTab=\"true\"] { color: palette(window-text); "
        "font-weight: bold; }" // アクティブ（標準の文字色）
    );

    auto *previewWidget = new GpuPreviewWidget(&windowState);
    auto *previewDock = new ads::CDockWidget(dockManager, "Preview");
    previewDock->setWidget(previewWidget);
    dockManager->addDockWidget(ads::TopDockWidgetArea, previewDock);

    this->timelineWidget = new TimelineWidget(windowState, 0);
    auto *timelineDock = new ads::CDockWidget(dockManager, "Timeline");
    timelineDock->setWidget(timelineWidget);
    dockManager->addDockWidget(ads::BottomDockWidgetArea, timelineDock);

#if SHOW_DEBUG_STREAMS
    this->debugStreamsWidget = new DebugStreamsWidget(&windowState);
    auto *debugStreamsDock = new ads::CDockWidget(dockManager, "DebugStreams");
    debugStreamsDock->setWidget(debugStreamsWidget);
    dockManager->addDockWidget(ads::RightDockWidgetArea, debugStreamsDock, previewDock->dockAreaWidget());
#endif

    // default timeline
    this->windowState.focusedTimeline = timelineWidget;

    QMenu *viewMenu = menuBar()->addMenu(tr("View"));
    viewMenu->addAction(previewDock->toggleViewAction());
    viewMenu->addAction(timelineDock->toggleViewAction());
    viewMenu->addAction(debugStreamsDock->toggleViewAction());
}

void MainWindow::markDirtyTimeline(size_t timelineId) {
    QTimer::singleShot(0, this, [this]() {
        timelineWidget->markRowsDirty();
        timelineWidget->update();
    });
}
} // namespace esotereel::window