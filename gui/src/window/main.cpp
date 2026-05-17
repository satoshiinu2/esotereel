#include "main.h"
#include "preview/camera.h"
#include "preview/wgpu_canvas.h"
#include "timeline/timeline.h"
#include <DockManager.h>
#include <QLabel>
#include <QTimer>

#define SHOW_DEBUG_STREAMS 0

#if SHOW_DEBUG_STREAMS
#include "debug_streams.h"
#endif

MainWindow::MainWindow(ClientNetworkHandler &network, QWidget *parent) : QMainWindow(parent) {
    this->windowState.network = &network;

    this->windowState.camera = new Camera();

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