#pragma once
#include "../wrapper/network.h"
#include "timeline.h"
#include <DockManager.h>
#include <QMainWindow>
#include <cstddef>

struct WindowGState {
    ClientNetworkHandler *network;
    TimelineWidget *focusedTimeline = nullptr;
};

class MainWindow : public QMainWindow {
    Q_OBJECT
  public:
    explicit MainWindow(ClientNetworkHandler &network, QWidget *parent = nullptr);

    void redrawTimeline(size_t timelineId) {
        timelineWidget->update();
    }

  protected:
    WindowGState windowState;

  private:
    ads::CDockManager *dockManager;
    TimelineWidget *timelineWidget;
};