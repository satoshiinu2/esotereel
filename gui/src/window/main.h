#pragma once
#include "../wrapper/network.fwd.h"
#include "../wrapper/project/camera.fwd.h"
#include <DockManager.h>
#include <QMainWindow>
#include <cstddef>

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

    void redrawTimeline(size_t timelineId);

  protected:
    WindowGState windowState;

  private:
    ads::CDockManager *dockManager;
    TimelineWidget *timelineWidget;
    DebugStreamsWidget *debugStreamsWidget;
};