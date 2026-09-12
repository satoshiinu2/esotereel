#pragma once
#include "ffi/Requests.h"
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

namespace dialog {
class SettingsDialog;
} // namespace dialog

struct WindowGState {
    ClientState *const network;
    TimelineWidget *focusedTimeline = nullptr;
    CameraInfo *camera{};
};

class MainWindow : public QMainWindow {
    Q_OBJECT
  public:
    explicit MainWindow(ClientState &network, QWidget *parent = nullptr);

    void markDirtyTimeline(TimelineId timelineId);

  protected:
    WindowGState windowState;

  private slots:
    void openSettingsDialog();

  private:
    ads::CDockManager *dockManager;
    TimelineWidget *timelineWidget;
    DebugStreamsWidget *debugStreamsWidget;
};

} // namespace esotereel::window