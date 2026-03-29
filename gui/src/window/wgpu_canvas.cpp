#include "wgpu_canvas.h"
#include "../util.h"
#include <QTimer>
#include <QWindow>

WgpuCanvasWidget::WgpuCanvasWidget() {
    // OS がこのウィンドウに直接描画するのを許可する（Qtのバックバッファをスキップ）
    setAttribute(Qt::WA_OpaquePaintEvent);
    setAttribute(Qt::WA_NativeWindow);
    setAttribute(Qt::WA_PaintOnScreen);
    setAttribute(Qt::WA_NoSystemBackground);
}

void WgpuCanvasWidget::showEvent(QShowEvent *event) {
    QWidget::showEvent(event);

    if (windowHandle()) {
        windowHandle()->setSurfaceType(QSurface::VulkanSurface);
    }

    int w = this->width() * this->devicePixelRatio();
    int h = this->height() * this->devicePixelRatio();

    this->wgpuutil = WWGpuUtil(this->winId(), getNativeDisplay(windowHandle()), w, h);

    renderTimer = new QTimer(this);
    connect(renderTimer, &QTimer::timeout, this, [this]() {
        if (this->wgpuutil.has_value()) {
            this->wgpuutil->renderFrame();
            update();
        }
    });
    renderTimer->start(16);
}

void WgpuCanvasWidget::paintEvent(QPaintEvent *event) {
    if (!this->wgpuutil.has_value()) {
        return;
    }

    // WId currentId = winId();
    // if (currentId != lastWinId) {
    //     this->wgpuutil->updateSurface(this->winId(), getNativeDisplay(windowHandle()));
    //     lastWinId = currentId;
    // }

    this->wgpuutil->renderFrame();
}

void WgpuCanvasWidget::resizeEvent(QResizeEvent *event) {
    QWidget::resizeEvent(event);

    int w = this->width() * this->devicePixelRatio();
    int h = this->height() * this->devicePixelRatio();

    if (this->wgpuutil.has_value()) {
        this->wgpuutil->updateSize(w, h);
    }
}
