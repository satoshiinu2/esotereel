#pragma once

#include "main.h"
#include <QScrollBar>
#include <QWidget>
#include <cstdint>
#include <map>
#include <vector>

class DebugStreamsWidget : public QWidget {
    Q_OBJECT

  public:
    explicit DebugStreamsWidget(WindowGState *windowState, QWidget *parent = nullptr);

  protected:
    void showEvent(QShowEvent *e) override;
    void paintEvent(QPaintEvent *e) override;

  private:
    QScrollBar *hScrollBar;
    std::map<uint32_t, std::vector<double>> streamMap;
    QTimer *renderTimer = nullptr;
    WindowGState *windowState;

    void updateMap();
};