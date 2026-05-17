#include "main.h"
#include "preview/wgpu_canvas.h"
#include "timeline/timeline.h"
#include <DockManager.h>
#include <QLabel>
#include <QTimer>
#include <QVector3D>
#include <qvectornd.h>

#define SHOW_DEBUG_STREAMS 0

#if SHOW_DEBUG_STREAMS
#include "debug_streams.h"
#endif

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

    this->dockManager = new ads::CDockManager(this);
    ads::CDockManager::setConfigFlag(ads::CDockManager::AlwaysShowTabs, false);

    // タブの文字色をスタイルシートで設定
    this->dockManager->setStyleSheet(
        "ads--CDockWidgetTab { color: palette(mid); font-size: 11px; }" // 非アクティブ（中間色）
        "ads--CDockWidgetTab[activeTab=\"true\"] { color: palette(window-text); "
        "font-weight: bold; }" // アクティブ（標準の文字色）
    );

    auto *previewWidget = new WgpuCanvasWidget(&windowState);
    auto *previewDock = new ads::CDockWidget(dockManager, "Preview");
    previewDock->setWidget(previewWidget);
    dockManager->addDockWidget(ads::TopDockWidgetArea, previewDock);

    this->timelineWidget = new TimelineWidget(&windowState, 0);
    auto *timelineDock = new ads::CDockWidget(dockManager, "Timeline");
    timelineDock->setWidget(timelineWidget);
    dockManager->addDockWidget(ads::BottomDockWidgetArea, timelineDock);

#if SHOW_DEBUG_STREAMS
    this->debugStreamsWidget = new DebugStreamsWidget(&windowState);
    auto *debugStreamsDock = new ads::CDockWidget(dockManager, "DebugStreams");
    debugStreamsDock->setWidget(debugStreamsWidget);
    dockManager->addDockWidget(ads::CenterDockWidgetArea, debugStreamsDock, previewDock->dockAreaWidget());
#endif

    // default timeline
    this->windowState.focusedTimeline = timelineWidget;
}

void MainWindow::redrawTimeline(size_t timelineId) {
    timelineWidget->update();
}