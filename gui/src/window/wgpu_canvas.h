#pragma once

#include "../wrapper/render.h"
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
    WgpuCanvasWidget();

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

  private:
    std::optional<WWGpuUtil> wgpuutil;
    WId lastWinId = 0;
    QTimer *renderTimer = nullptr;
};