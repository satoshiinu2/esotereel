#include "wgpu_canvas.h"
#include "../util.h"

WgpuCanvasWidget::WgpuCanvasWidget() {
    // OS がこのウィンドウに直接描画するのを許可する（Qtのバックバッファをスキップ）
    setAttribute(Qt::WA_PaintOnScreen);
    setAttribute(Qt::WA_NoSystemBackground);
    setAttribute(Qt::WA_OpaquePaintEvent);
}

void WgpuCanvasWidget::showEvent(QShowEvent *event) {
    QWidget::showEvent(event);

    int w = this->width() * this->devicePixelRatio();
    int h = this->height() * this->devicePixelRatio();

    this->wgpuutil = WWGpuUtil(this->winId(), getNativeDisplay(), w, h);
}

void WgpuCanvasWidget::paintEvent(QPaintEvent *event) {
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

void WgpuCanvasWidget::updateWgpuSurface() {
    if (this->wgpuutil.has_value()) {
        this->wgpuutil->updateSurface(this->winId(), getNativeDisplay());
    }
}
