#include "wgpu_canvas.h"
#include "../../util.h"
#include "../../wrapper/project/project.h"
#include "../../wrapper/project/timeline.h"
#include "../../wrapper/wgpuutil.h"
#include "../timeline/timeline.h"
#include <QDebug>
#include <QPointer>
#include <QVBoxLayout>
#include <stdexcept>

// ============================ WgpuRenderWindow ============================

WgpuRenderWindow::WgpuRenderWindow(WindowGState *windowState, QWindow *parent)
    : QWindow(parent), windowState(windowState) {
    setSurfaceType(QSurface::VulkanSurface);

    renderTimer = new QTimer(this);
    connect(renderTimer, &QTimer::timeout, this, [this]() { requestRender(); });
    renderTimer->start(16);
}

void WgpuRenderWindow::exposeEvent(QExposeEvent *event) {
    QWindow::exposeEvent(event);
    if (isExposed()) {
        ensureInitialized();
    } else {
        // 遷移中は描画もサーフェス操作も止める
        renderTimer->stop();
    }
}
void WgpuRenderWindow::resizeEvent(QResizeEvent *event) {
    QWindow::resizeEvent(event);

    if (!wgpuutil.has_value()) {
        return;
    }

    int w = this->width() * this->devicePixelRatio();
    int h = this->height() * this->devicePixelRatio();

    // Qt/glib のイベントディスパッチの奥深くから呼ばれるため、
    // ここから先で例外を外に漏らすとC言語(glib)のスタックフレームを
    // アンワインドすることになり未定義動作になる。必ずここで握りつぶす。
    QPointer<WgpuRenderWindow> self(this);
    QTimer::singleShot(0, this, [self]() {
        if (!self || !self->wgpuutil.has_value()) {
            return;
        }
        int w = self->width() * self->devicePixelRatio();
        int h = self->height() * self->devicePixelRatio();
        try {
            self->wgpuutil->updateSize(w, h);
        } catch (const std::exception &e) {
            qWarning() << "WgpuRenderWindow: updateSize failed, abandoning surface:" << e.what();
            self->wgpuutil->abandon();
            self->wgpuutil.reset();
        }
    });
}

bool WgpuRenderWindow::event(QEvent *ev) {
    if (ev->type() == QEvent::PlatformSurface) {
        auto *surfaceEvent = static_cast<QPlatformSurfaceEvent *>(ev);
        switch (surfaceEvent->surfaceEventType()) {
        case QPlatformSurfaceEvent::SurfaceAboutToBeDestroyed:
            // ネイティブサーフェス(wl_surfaceなど)が実際に破棄される前に、
            // Vulkan側の参照を先に手放しておく。ここでdropしないと、
            // 破棄済みのサーフェスに対してVulkanが操作を続けてしまい、
            // ERROR_SURFACE_LOST_KHRやプロトコルエラーの原因になる。
            wgpuutil.reset();
            break;
        case QPlatformSurfaceEvent::SurfaceCreated:
            if (!isExposed()) {
                // まだcompositor側の準備が終わっていない可能性がある
                break;
            }
            // ドッキングのフローティング化などで親トップレベルが変わると、
            // ネイティブサーフェスが作り直されることがある。
            // 既存のwgpuutilがあれば新しいハンドルに張り替え、
            // なければ通常の初期化を試みる。
            if (wgpuutil.has_value()) {
                try {
                    NativeWindowHandle handle = getNativeWindowHandle(this);
                    wgpuutil->updateSurface(handle);
                } catch (const std::exception &e) {
                    qWarning() << "WgpuRenderWindow: updateSurface failed, abandoning surface:" << e.what();
                    wgpuutil->abandon();
                    wgpuutil.reset();
                }
            } else {
                ensureInitialized();
            }
            break;
        default:
            break;
        }
    }

    return QWindow::event(ev);
}

void WgpuRenderWindow::ensureInitialized() {
    if (wgpuutil.has_value()) {
        return;
    }
    if (!isExposed()) {
        return;
    }

    int w = this->width() * this->devicePixelRatio();
    int h = this->height() * this->devicePixelRatio();
    if (w == 0 || h == 0) {
        return;
    }

    try {
        NativeWindowHandle handle = getNativeWindowHandle(this);
        wgpuutil = WGpuUtil(windowState->network, handle, w, h);
    } catch (const std::exception &e) {
        qWarning() << "WgpuRenderWindow: failed to initialize surface, abandoning:" << e.what();
        if (wgpuutil.has_value()) {
            wgpuutil->abandon();
        }
        wgpuutil.reset();
    }
}

bool WgpuRenderWindow::requestRender() {
    ensureInitialized();

    if (!wgpuutil.has_value()) {
        return false;
    }

    auto project = windowState->network->getProject();
    auto focusedTimelineWidget = windowState->focusedTimeline;
    if (!project.isValid() || !focusedTimelineWidget) {
        return false;
    }

    Timeline timeline = project.timelineOf(focusedTimelineWidget->timelineIdx);
    int64_t currentFrame = focusedTimelineWidget->playhead;

    // 同上の理由でここも必ず握りつぶす。QTimer::timeoutからの呼び出しなので
    // resizeEventほど深いCコールスタックは挟まないことが多いが、
    // 予防的に統一しておく。
    try {
        wgpuutil->renderFrame(timeline, windowState->camera, currentFrame);
    } catch (const std::exception &e) {
        qWarning() << "WgpuRenderWindow: renderFrame failed, abandoning surface:" << e.what();
        wgpuutil->abandon();
        wgpuutil.reset();
        return false;
    }
    return true;
}

// ============================ WgpuCanvasWidget ============================

WgpuCanvasWidget::WgpuCanvasWidget(WindowGState *windowState) {
    renderWindow = new WgpuRenderWindow(windowState);
    container = QWidget::createWindowContainer(renderWindow, this);

    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    layout->addWidget(container);
}
