#pragma once
#include <DockManager.h>
#include <QMainWindow>

class MainWindow : public QMainWindow {
    Q_OBJECT
  public:
    explicit MainWindow(QWidget *parent = nullptr);

  private:
    ads::CDockManager *dockManager;
};