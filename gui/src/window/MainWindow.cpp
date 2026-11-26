#include "MainWindow.h"
#include "WidgetManager.h"
#include "ads_globals.h"
#include "dialog/project/RecoveryDialog.h"
#include "dialog/settings/AppSettingsDialog.h"
#include "dialog/settings/SettingsDialog.h"
#include "esotereel_gui_helper.h"
#include "ffi/ProjectFiles.h"
#include "ffi/Requests.h"
#include "ffi/project/CommandQueue.h"
#include "widget/assets/AssetsPanel.h"
#include "widget/clip_property/ClipPropertiesPanel.h"
#ifdef ESOTEREEL_ENABLE_GPU_PREVIEW
#include "widget/preview/GpuPreviewWidget.h"
#endif
#include "widget/timeline/TimelineCanvasWidget.h"
#include "widget/timeline/TimelineWidget.h"
#include <DockManager.h>
#include <QAction>
#include <QCheckBox>
#include <QDir>
#include <QFileDialog>
#include <QHBoxLayout>
#include <QLabel>
#include <QMenuBar>
#include <QMessageBox>
#include <QPushButton>
#include <QStatusBar>
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
#ifdef ESOTEREEL_ENABLE_GPU_PREVIEW
    widgetManager.createWidget<GpuPreviewWidget>("Preview", ads::TopDockWidgetArea, &windowState);
#else
    auto *previewPlaceholder = new QLabel(
        tr("GPU preview is disabled.\nConfigure with -DESOTEREEL_ENABLE_GPU_PREVIEW=ON to enable it."));
    previewPlaceholder->setAlignment(Qt::AlignCenter);
    widgetManager.createDock(previewPlaceholder, "Preview", ads::TopDockWidgetArea);
#endif

    // Timeline widget
    this->timelineWidget =
        widgetManager.createWidget<TimelineWidget>("Timeline", ads::BottomDockWidgetArea, windowState, 0);

#if SHOW_DEBUG_STREAMS
    // Debug Streams widget (hidden by default)
    this->debugStreamsWidget = widgetManager.createWidgetWithReferenceAndVisible<DebugStreamsWidget>(
        "DebugStreams", ads::RightDockWidgetArea, widgetManager.getDock("Preview"), false, &windowState);
#endif

    // Clip Properties panel (requires CommandQueue)
    this->commandQueue = new esotereel::CommandQueue(windowState.state);
    this->clipPropertiesPanel = new ClipPropertiesPanel(windowState.state, *this->commandQueue);
    widgetManager.createDock(clipPropertiesPanel, "Clip Properties", ads::RightDockWidgetArea,
                             widgetManager.getDock("Preview"));

    // Assets panel (synced asset table from the server)
    this->assetsPanel = new AssetsPanel(windowState.state);
    widgetManager.createDock(assetsPanel, "Assets", ads::RightDockWidgetArea,
                             widgetManager.getDock("Clip Properties"));

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
    QMenu *fileMenu = menuBar()->addMenu(tr("File"));
    QAction *openAction = fileMenu->addAction(tr("Open Project..."));
    connect(openAction, &QAction::triggered, this, &MainWindow::openProject);
    QAction *saveAction = fileMenu->addAction(tr("Save Project"));
    connect(saveAction, &QAction::triggered, this, &MainWindow::saveProject);
    QAction *saveAsAction = fileMenu->addAction(tr("Save Project As..."));
    connect(saveAsAction, &QAction::triggered, this, &MainWindow::saveProjectAs);
    fileMenu->addSeparator();
    QAction *recoveryAction = fileMenu->addAction(tr("Autosave Recovery..."));
    connect(recoveryAction, &QAction::triggered, this, &MainWindow::showRecoveryDialog);

    QMenu *viewMenu = menuBar()->addMenu(tr("View"));
    widgetManager.addToMenu(viewMenu, widgetManager.getDock("Preview"));
    widgetManager.addToMenu(viewMenu, widgetManager.getDock("Timeline"));
    widgetManager.addToMenu(viewMenu, widgetManager.getDock("DebugStreams"));
    widgetManager.addToMenu(viewMenu, widgetManager.getDock("Clip Properties"));
    widgetManager.addToMenu(viewMenu, widgetManager.getDock("Assets"));

    QMenu *toolsMenu = menuBar()->addMenu(tr("Tools"));
    QAction *settingsAction = toolsMenu->addAction(tr("Settings"));
    connect(settingsAction, &QAction::triggered, this, &MainWindow::openSettingsDialog);
    QAction *appSettingsAction = toolsMenu->addAction(tr("App Settings..."));
    connect(appSettingsAction, &QAction::triggered, this, &MainWindow::openAppSettingsDialog);
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

    // バックグラウンドスレッドの非同期エラーを定期的に回収し、無言で
    // 失敗し続ける状態を避ける（checkAsyncError 参照）。
    asyncErrorTimer = new QTimer(this);
    connect(asyncErrorTimer, &QTimer::timeout, this, &MainWindow::checkAsyncError);
    asyncErrorTimer->start(1000);
}

MainWindow::~MainWindow() {
    delete commandQueue;
}

void MainWindow::markDirtyTimeline(TimelineId timelineId) {
    QTimer::singleShot(0, this, [this]() {
        timelineWidget->canvas->markRowsDirty();
        timelineWidget->canvas->update();
        clipPropertiesPanel->refreshChangedFields();
    });
}

void MainWindow::refreshAssets() {
    QTimer::singleShot(0, this, [this]() {
        if (assetsPanel) {
            assetsPanel->reload();
        }
    });
}

void MainWindow::openSettingsDialog() {
    dialog::SettingsDialog settingsDialog(windowState, this);
    settingsDialog.exec();
}

void MainWindow::openAppSettingsDialog() {
    dialog::AppSettingsDialog appSettingsDialog(windowState, this);
    appSettingsDialog.exec();
}

void MainWindow::showRecoveryDialog() {
    dialog::RecoveryDialog recoveryDialog(windowState, this);
    recoveryDialog.exec();
}

void MainWindow::openProject() {
    const QString dir = QFileDialog::getExistingDirectory(this, tr("Open Project Folder"));
    if (dir.isEmpty()) {
        return;
    }
    // まずバージョンだけ確認し、判定に応じて段階表示してから開く。
    windowState.state->requests().inspectProject(dir);
    pollForInspection(dir, 0);
}

void MainWindow::pollForInspection(const QString &path, int attempts) {
    auto result = ProjectFiles::takeInspection(windowState.state);
    if (result.isOk()) {
        auto inspection = result.unwrap();
        if (inspection.has_value() && inspection->path == path) {
            confirmAndOpenProject(*inspection);
            return;
        }
    }
    if (attempts >= 50) {
        statusBar()->showMessage(tr("Project inspection timed out: %1").arg(path));
        return;
    }
    QTimer::singleShot(100, this, [this, path, attempts]() { pollForInspection(path, attempts + 1); });
}

void MainWindow::confirmAndOpenProject(const ProjectInspection &inspection) {
    const QString version = tr("%1.%2.%3")
                                .arg(inspection.versionMajor)
                                .arg(inspection.versionMinor)
                                .arg(inspection.versionRevision);
    switch (inspection.status) {
    case 0: // Compatible
    case 1: // NewerRevision: そのまま開く
        windowState.state->requests().openProject(inspection.path, false);
        break;
    case 2: { // NewerMinor: 警告＋バックアップボタン
        QMessageBox box(this);
        box.setIcon(QMessageBox::Warning);
        box.setWindowTitle(tr("Newer Project Version"));
        box.setText(tr("This project was saved by a newer version (%1). "
                       "Opening it here may lose newer data when saved.")
                        .arg(version));
        auto *backupButton = box.addButton(tr("Create Backup and Open"), QMessageBox::AcceptRole);
        auto *openButton = box.addButton(tr("Open Without Backup"), QMessageBox::AcceptRole);
        box.addButton(QMessageBox::Cancel);
        box.exec();
        if (box.clickedButton() == backupButton) {
            windowState.state->requests().openProject(inspection.path, true);
        } else if (box.clickedButton() == openButton) {
            windowState.state->requests().openProject(inspection.path, false);
        }
        break;
    }
    default: { // NewerMajor: チェックボックス確認付きの強い警告＋バックアップボタン
        QDialog dialog(this);
        dialog.setWindowTitle(tr("Much Newer Project Version"));
        auto *layout = new QVBoxLayout(&dialog);
        auto *label = new QLabel(tr("This project was saved by a much newer major version (%1). "
                                    "This build may not understand its format. "
                                    "Opening it is strongly discouraged.")
                                     .arg(version));
        label->setWordWrap(true);
        layout->addWidget(label);
        auto *acknowledge = new QCheckBox(tr("I understand the risk and want to open it anyway"));
        layout->addWidget(acknowledge);
        auto *buttons = new QHBoxLayout();
        auto *backupButton = new QPushButton(tr("Create Backup and Open"));
        auto *openButton = new QPushButton(tr("Open Without Backup"));
        auto *cancelButton = new QPushButton(tr("Cancel"));
        backupButton->setEnabled(false);
        openButton->setEnabled(false);
        connect(acknowledge, &QCheckBox::toggled, backupButton, &QPushButton::setEnabled);
        connect(acknowledge, &QCheckBox::toggled, openButton, &QPushButton::setEnabled);
        bool withBackup = true;
        connect(backupButton, &QPushButton::clicked, &dialog,
                [&dialog, &withBackup]() {
                    withBackup = true;
                    dialog.accept();
                });
        connect(openButton, &QPushButton::clicked, &dialog,
                [&dialog, &withBackup]() {
                    withBackup = false;
                    dialog.accept();
                });
        connect(cancelButton, &QPushButton::clicked, &dialog, &QDialog::reject);
        buttons->addWidget(backupButton);
        buttons->addWidget(openButton);
        buttons->addWidget(cancelButton);
        layout->addLayout(buttons);
        if (dialog.exec() == QDialog::Accepted) {
            windowState.state->requests().openProject(inspection.path, withBackup);
        }
        break;
    }
    }
}

void MainWindow::saveProject() {
    windowState.state->requests().saveProject(false);
}

void MainWindow::saveProjectAs() {
    const QString dir = QFileDialog::getExistingDirectory(this, tr("Save Project As"));
    if (dir.isEmpty()) {
        return;
    }
    bool overwrite = false;
    const bool nonEmpty =
        !QDir(dir).entryList(QDir::AllEntries | QDir::NoDotAndDotDot).isEmpty();
    if (nonEmpty) {
        const auto answer = QMessageBox::question(
            this, tr("Folder Not Empty"),
            tr("The selected folder is not empty. Save anyway and overwrite its contents?"),
            QMessageBox::Yes | QMessageBox::No, QMessageBox::No);
        if (answer != QMessageBox::Yes) {
            return;
        }
        overwrite = true;
    }
    windowState.state->requests().saveProjectAs(dir, overwrite);
}

void MainWindow::checkAsyncError() {
    auto result = windowState.state->takeAsyncError();
    if (result.isError()) {
        auto err = result.getError();
        qWarning() << "MainWindow: async error:" << QString::fromStdString(*err);
        statusBar()->showMessage(QString::fromStdString(*err));
    }
}
} // namespace esotereel::window