#include "wgpu_canvas.h"
#include "../../util.h"
#include "../../wrapper/project/project.h"
#include "../../wrapper/project/timeline.h"
#include "../../wrapper/wgpuutil.h"
#include "../timeline/timeline.h"
#include "render_worker.h"
#include <QDebug>
#include <QEvent>
#include <QPlatformSurfaceEvent>
#include <QPointer>
#include <QThread>
#include <QVBoxLayout>

WgpuRenderWindow::WgpuRenderWindow(WindowGState *windowState, QWindow *parent)
    : QWindow(parent), windowState(windowState) {
    setSurfaceType(QSurface::VulkanSurface); // もしくはOpenGLSurface/RasterSurface、wgpuバックエンドに合わせる
    // WA_NativeWindow等のQWidget属性は不要(QWindowは元々ネイティブ)

    m_thread = new QThread(this);
    m_worker = new WgpuRenderWorker(windowState);
    m_worker->moveToThread(m_thread);
    connect(m_thread, &QThread::finished, m_worker, &QObject::deleteLater);
    m_thread->start();

    connect(this, &WgpuRenderWindow::requestRender, m_worker, &WgpuRenderWorker::renderFrame, Qt::QueuedConnection);
    connect(this, &WgpuRenderWindow::requestResize, m_worker, &WgpuRenderWorker::resize, Qt::QueuedConnection);
    connect(this, &WgpuRenderWindow::requestSurfaceUpdate, m_worker, &WgpuRenderWorker::updateSurface,
            Qt::BlockingQueuedConnection);
    connect(this, &WgpuRenderWindow::requestSurfaceDestroy, m_worker, &WgpuRenderWorker::destroySurface,
            Qt::BlockingQueuedConnection);
    connect(this, &WgpuRenderWindow::requestInit, m_worker, &WgpuRenderWorker::initialize, Qt::QueuedConnection);
    connect(m_worker, &WgpuRenderWorker::initFailed, this, &WgpuRenderWindow::onInitFailed, Qt::QueuedConnection);
    connect(m_worker, &WgpuRenderWorker::frameFailed, this, &WgpuRenderWindow::onFrameFailed, Qt::QueuedConnection);

    renderTimer = new QTimer(this);
    connect(renderTimer, &QTimer::timeout, this, &WgpuRenderWindow::renderFrame);
    renderTimer->start(16);
}

WgpuRenderWindow::~WgpuRenderWindow() {
    renderTimer->stop();
    if (m_initialized) {
        QMetaObject::invokeMethod(m_worker, "destroySurface", Qt::BlockingQueuedConnection);
        m_initialized = false;
    }
    m_thread->quit();
    m_thread->wait();
    delete m_worker;
}

void WgpuRenderWindow::renderFrame() {
    ensureInitialized();

    auto project = windowState->network->getProject();
    auto focusedTimelineWidget = windowState->focusedTimeline;
    if (!project.isValid() || !focusedTimelineWidget)
        return;

    Timeline timeline = project.timelineOf(focusedTimelineWidget->timelineIdx);
    CameraInfo *camera = windowState->camera;
    int64_t currentFrame = focusedTimelineWidget->playhead;

    emit requestRender(timeline, camera, currentFrame);
}

void WgpuRenderWindow::ensureInitialized() {
    qDebug() << "ensureInitialized called, initialized=" << m_initialized << "exposed=" << isExposed()
             << "size=" << width() << height();

    if (m_initialized || !isExposed())
        return;

    int w = this->width() * this->devicePixelRatio();
    int h = this->height() * this->devicePixelRatio();
    if (w == 0 || h == 0)
        return;

    NativeWindowHandle handle = getNativeWindowHandle(this); // QWindow*版が既にある
    m_initialized = true;
    emit requestInit(handle, w, h);
}

void WgpuRenderWindow::onInitFailed(QString reason) {
    qWarning() << "WgpuRenderWindow: init failed:" << reason;
    m_initialized = false;
}

void WgpuRenderWindow::onFrameFailed(QString reason) {
    qWarning() << "WgpuRenderWindow: frame failed:" << reason;
    // m_initialized = false; // 次のrenderFrameでensureInitializedが再初期化を試みる
}

void WgpuRenderWindow::exposeEvent(QExposeEvent *event) {
    Q_UNUSED(event);
    qDebug() << "exposeEvent" << isExposed() << width() << height();
    ensureInitialized();
}

void WgpuRenderWindow::resizeEvent(QResizeEvent *event) {
    QWindow::resizeEvent(event);
    if (!m_initialized)
        return;

    int w = this->width() * this->devicePixelRatio();
    int h = this->height() * this->devicePixelRatio();
    if (w == 0 || h == 0)
        return;

    emit requestResize(w, h); // QueuedConnectionで十分(連続リサイズは最終値だけ反映されればOK)
}

bool WgpuRenderWindow::event(QEvent *ev) {
    if (ev->type() == QEvent::PlatformSurface) {
        auto *surfaceEvent = static_cast<QPlatformSurfaceEvent *>(ev);
        if (surfaceEvent->surfaceEventType() == QPlatformSurfaceEvent::SurfaceAboutToBeDestroyed) {
            if (m_initialized) {
                emit requestSurfaceDestroy();
                // m_initialized はfalseにしない。wgpuutil自体(device等)は生きてる。
            }
        } else if (surfaceEvent->surfaceEventType() == QPlatformSurfaceEvent::SurfaceCreated) {
            if (m_initialized) {
                NativeWindowHandle handle = getNativeWindowHandle(this);
                emit requestSurfaceUpdate(handle); // 常にattachSurface経路
            } else {
                ensureInitialized(); // 起動直後、wgpuutil自体がまだない場合のみ
            }
        }
    }
    return QWindow::event(ev);
}