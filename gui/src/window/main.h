#pragma once
#include "timeline.h"
#include <DockManager.h>
#include <QMainWindow>
#include <cstddef>

class MainWindow : public QMainWindow {
    Q_OBJECT
  public:
    explicit MainWindow(QWidget *parent = nullptr);

    void onUpdateTimeline(size_t timelineId) {
        timelineWidget->update();
    }

  private:
    ads::CDockManager *dockManager;
    TimelineWidget *timelineWidget;
};