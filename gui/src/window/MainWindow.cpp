#include "MainWindow.h"
#include "WidgetManager.h"
#include "ads_globals.h"
#include "dialog/settings/SettingsDialog.h"
#include "esotereel_gui_helper.h"
#include "ffi/Requests.h"
#include "ffi/project/CommandQueue.h"
#include "widget/clip_property/ClipPropertiesPanel.h"
#include "widget/preview/GpuPreviewWidget.h"
#include "widget/timeline/TimelineCanvasWidget.h"
#include "widget/timeline/TimelineWidget.h"
#include <DockManager.h>
#include <QAction>
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

void MainWindow::setupWindowState() {
    this->windowState.camera = new CameraInfo{};
    this->windowState.camera->position = QVector3D(0, 0, 0);
    this->windowState.camera->rotation = QVector3D(0, 0, 0);
    this->windowState.camera->is_orthographic = true;
    this->windowState.camera->orthographic_direction = Direction::Front;
    this->windowState.camera->scale_factor = 1.0;
    this->windowState.camera->fov = 60.0;
}

void MainWindow::setupDockManager() {
    ads::CDockManager::setConfigFlag(ads::CDockManager::AlwaysShowTabs, false);
    this->dockManager = new ads::CDockManager(this);
    setCentralWidget(this->dockManager);

    // タブの文字色をスタイルシートで設定
    this->dockManager->setStyleSheet(
        "ads--CDockWidgetTab { color: palette(mid); font-size: 11px; }" // 非アクティブ（中間色）
        "ads--CDockWidgetTab[activeTab=\"true\"] { color: palette(window-text); "
        "font-weight: bold; }" // アクティブ（標準の文字色）
    );
}

void MainWindow::setupWidgets(WidgetManager &widgetManager) {
    // Preview widget
    widgetManager.createWidget<GpuPreviewWidget>("Preview", ads::TopDockWidgetArea, &windowState);

    // Timeline widget
    this->timelineWidget = widgetManager.createWidget<TimelineWidget>("Timeline", ads::BottomDockWidgetArea, windowState, 0);

#if SHOW_DEBUG_STREAMS
    // Debug Streams widget (hidden by default)
    this->debugStreamsWidget = widgetManager.createWidgetWithReferenceAndVisible<DebugStreamsWidget>(
        "DebugStreams", ads::RightDockWidgetArea, widgetManager.getDock("Preview"), false, &windowState);
#endif

    // Clip Properties panel (requires CommandQueue)
    this->commandQueue = new esotereel::CommandQueue(windowState.state);
    this->clipPropertiesPanel = new ClipPropertiesPanel(windowState.state, *this->commandQueue);
    widgetManager.createDock(clipPropertiesPanel, "Clip Properties", ads::RightDockWidgetArea, widgetManager.getDock("Preview"));

    // default timeline
    this->windowState.focusedTimeline = timelineWidget;
}

void MainWindow::setupConnections() {
    // Connect timeline selection to clip properties panel
    connect(timelineWidget->canvas, &TimelineCanvasWidget::clipSelectionChanged, this,
            [this](TimelineId timelineId, const std::vector<ClipId> &clipIds) {
                clipPropertiesPanel->setClips(timelineId, clipIds);
            });
}

void MainWindow::setupMenus(WidgetManager &widgetManager) {
    QMenu *viewMenu = menuBar()->addMenu(tr("View"));
    widgetManager.addToMenu(viewMenu, widgetManager.getDock("Preview"));
    widgetManager.addToMenu(viewMenu, widgetManager.getDock("Timeline"));
    widgetManager.addToMenu(viewMenu, widgetManager.getDock("DebugStreams"));
    widgetManager.addToMenu(viewMenu, widgetManager.getDock("Clip Properties"));

    QMenu *toolsMenu = menuBar()->addMenu(tr("Tools"));
    QAction *settingsAction = toolsMenu->addAction(tr("Settings"));
    connect(settingsAction, &QAction::triggered, this, &MainWindow::openSettingsDialog);
}

MainWindow::MainWindow(ClientState &network, QWidget *parent) : QMainWindow(parent), windowState{&network} {
    setupWindowState();
    resize(1280, 720);
    setWindowTitle("Esotereel");

    setupDockManager();

    WidgetManager widgetManager(dockManager);
    setupWidgets(widgetManager);
    setupConnections();
    setupMenus(widgetManager);
}

MainWindow::~MainWindow() {
    delete commandQueue;
}

void MainWindow::markDirtyTimeline(TimelineId timelineId) {
    QTimer::singleShot(0, this, [this]() {
        timelineWidget->canvas->markRowsDirty();
        timelineWidget->canvas->update();
    });
}

void MainWindow::openSettingsDialog() {
    dialog::SettingsDialog settingsDialog(windowState, this);
    settingsDialog.exec();
}
} // namespace esotereel::window