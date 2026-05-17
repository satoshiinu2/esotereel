#pragma once

#include "../../wrapper/wgpuutil.h"
#include "../main.h"
#include <QEvent>
#include <QPainter>
#include <QScrollBar>
#include <QWidget>
#include <cmath>
#include <optional>
#include <qpair.h>
#include <qpoint.h>

class WgpuCanvasWidget : public QWidget {
    Q_OBJECT

  public:
    explicit WgpuCanvasWidget(WindowGState *windowState);

    WgpuCanvasWidget(const WgpuCanvasWidget &) = delete;
    WgpuCanvasWidget(WgpuCanvasWidget &&) = delete;
    WgpuCanvasWidget &operator=(const WgpuCanvasWidget &) = delete;
    WgpuCanvasWidget &operator=(WgpuCanvasWidget &&) = delete;

  protected:
    void showEvent(QShowEvent *e) override;
    void paintEvent(QPaintEvent *e) override;
    void resizeEvent(QResizeEvent *e) override;

    QPaintEngine *paintEngine() const override {
        return nullptr;
    }

    bool tryRender();

  private:
    WindowGState *windowState;
    std::optional<WGpuUtil> wgpuutil;
    WId lastWinId = 0;
    QTimer *renderTimer = nullptr;
};