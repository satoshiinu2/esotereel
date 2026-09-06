#pragma once
#include "ffi/Requests.h"
#include <DockManager.h>
#include <QMainWindow>
#include <cstddef>

namespace esotereel_gui_helper {
struct CameraInfo;
enum class Direction;
} // namespace esotereel_gui_helper

namespace esotereel {
class ClientNetworkHandler;
}

namespace esotereel::window {
using CameraInfo = esotereel_gui_helper::CameraInfo;
using Direction = esotereel_gui_helper::Direction;

class TimelineWidget;

class DebugStreamsWidget;

struct WindowGState {
    ClientNetworkHandler *network;
    TimelineWidget *focusedTimeline = nullptr;
    CameraInfo *camera{};
};

class MainWindow : public QMainWindow {
    Q_OBJECT
  public:
    explicit MainWindow(ClientNetworkHandler &network, QWidget *parent = nullptr);

    void markDirtyTimeline(TimelineId timelineId);

  protected:
    WindowGState windowState;

  private:
    ads::CDockManager *dockManager;
    TimelineWidget *timelineWidget;
    DebugStreamsWidget *debugStreamsWidget;
};

} // namespace esotereel::window