#pragma once
#include "ffi/Requests.h"
#include "ffi/project/CommandQueue.h"
#include <DockManager.h>
#include <QAction>
#include <QMainWindow>
#include <cstddef>

namespace esotereel_gui_helper {
struct CameraInfo;
enum class Direction;
} // namespace esotereel_gui_helper

namespace esotereel {
class ClientState;
}

namespace esotereel::window {
using CameraInfo = esotereel_gui_helper::CameraInfo;
using Direction = esotereel_gui_helper::Direction;

class TimelineWidget;

class DebugStreamsWidget;

class ClipPropertiesPanel;

class WidgetManager;

namespace dialog {
class SettingsDialog;
} // namespace dialog

struct WindowGState {
    ClientState *const state;
    TimelineWidget *focusedTimeline = nullptr;
    CameraInfo *camera{};
};

class MainWindow : public QMainWindow {
    Q_OBJECT
  public:
    explicit MainWindow(ClientState &network, QWidget *parent = nullptr);
    ~MainWindow();

    void markDirtyTimeline(TimelineId timelineId);

  protected:
    WindowGState windowState;

  private slots:
    void openSettingsDialog();

  private:
    void setupWindowState();
    void setupDockManager();
    void setupWidgets(WidgetManager &widgetManager);
    void setupConnections();
    void setupMenus(WidgetManager &widgetManager);

    ads::CDockManager *dockManager;
    TimelineWidget *timelineWidget;
    DebugStreamsWidget *debugStreamsWidget;
    ClipPropertiesPanel *clipPropertiesPanel;
    esotereel::CommandQueue *commandQueue;
};

} // namespace esotereel::window