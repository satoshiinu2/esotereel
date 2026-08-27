#pragma once

#include "window/MainWindow.h"
#include <QScrollBar>
#include <QWidget>
#include <cstdint>
#include <map>
#include <vector>

namespace esotereel::window {
class DebugStreamsWidget : public QWidget {
    Q_OBJECT

  public:
    explicit DebugStreamsWidget(WindowGState *windowState, QWidget *parent = nullptr);

  protected:
    void showEvent(QShowEvent *e) override;
    void paintEvent(QPaintEvent *e) override;
    void resizeEvent(QResizeEvent *e) override;

  private:
    QScrollBar *hScrollBar;
    std::map<uint32_t, std::vector<double>> streamMap;
    QTimer *renderTimer = nullptr;
    WindowGState *windowState;

    void updateMap();
};
} // namespace esotereel::window