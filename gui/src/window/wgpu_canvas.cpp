#include "wgpu_canvas.h"
#include "../util.h"
#include "../wrapper/project/project.h"
#include "timeline.h"
#include <QTimer>
#include <QWindow>
#include <cstdint>

WgpuCanvasWidget::WgpuCanvasWidget(WindowGState *windowState) : windowState(windowState) {
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

    this->wgpuutil = WGpuUtil(windowState->network, this->winId(), getNativeDisplay(windowHandle()), w, h);

    renderTimer = new QTimer(this);
    connect(renderTimer, &QTimer::timeout, this, [this]() {
        update();
    });
    renderTimer->start(16);
}

void WgpuCanvasWidget::paintEvent(QPaintEvent *event) {
    this->tryRender();
}

bool WgpuCanvasWidget::tryRender() {
    if (this->wgpuutil.has_value()) {
        auto project = windowState->network->getProject();

        auto focusedTimelineWidget = this->windowState->focusedTimeline;
        if (!project.isValid() || !focusedTimelineWidget) {
            return false;
        }

        Timeline timeline = project.timelineOf(focusedTimelineWidget->timelineIdx);

        int64_t currentFrame = focusedTimelineWidget->playhead;

        this->wgpuutil->renderFrame(timeline, currentFrame);
        return true;
    }
    return false;
}

void WgpuCanvasWidget::resizeEvent(QResizeEvent *event) {
    QWidget::resizeEvent(event);

    int w = this->width() * this->devicePixelRatio();
    int h = this->height() * this->devicePixelRatio();

    if (this->wgpuutil.has_value()) {
        this->wgpuutil->updateSize(w, h);
    }
}
