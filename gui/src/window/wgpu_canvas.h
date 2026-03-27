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
    void showEvent(QShowEvent *event) override;
    void paintEvent(QPaintEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;

    QPaintEngine *paintEngine() const override {
        return nullptr;
    }
    bool event(QEvent *event) override {
        if (event->type() == QEvent::WinIdChange) {
            updateWgpuSurface();
        }
        return QWidget::event(event);
    }

  private:
    std::optional<WWGpuUtil> wgpuutil;

    void updateWgpuSurface();
};