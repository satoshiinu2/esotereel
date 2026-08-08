#include "wgpu_canvas.h"
#include "../../util.h"
#include "../../wrapper/network.h"
#include "../../wrapper/project/project.h"
#include "../../wrapper/project/timeline.h"
#include "../timeline/timeline.h"
#include "render_worker.h"
#include <QDebug>
#include <QEvent>
#include <QPlatformSurfaceEvent>
#include <QPointer>
#include <QThread>
#include <QVBoxLayout>

GpuPreviewWidget::GpuPreviewWidget(WindowGState *windowState, QWidget *parent)
    : QWidget(parent), windowState(windowState) {
    m_thread = new QThread(this);
    m_worker = new GpuRenderWorker(windowState);
    m_worker->moveToThread(m_thread);
    connect(m_thread, &QThread::finished, m_worker, &QObject::deleteLater);
    m_thread->start();

    connect(this, &GpuPreviewWidget::requestInit, m_worker, &GpuRenderWorker::initialize, Qt::QueuedConnection);
    connect(this, &GpuPreviewWidget::requestResize, m_worker, &GpuRenderWorker::resize, Qt::QueuedConnection);
    connect(this, &GpuPreviewWidget::requestRender, m_worker, &GpuRenderWorker::renderFrame, Qt::QueuedConnection);
    connect(m_worker, &GpuRenderWorker::frameReady, this, &GpuPreviewWidget::onFrameReady, Qt::QueuedConnection);
    connect(m_worker, &GpuRenderWorker::initFailed, this, &GpuPreviewWidget::onInitFailed, Qt::QueuedConnection);
    connect(m_worker, &GpuRenderWorker::frameFailed, this, &GpuPreviewWidget::onFrameFailed, Qt::QueuedConnection);

    renderTimer = new QTimer(this);
    connect(renderTimer, &QTimer::timeout, this, &GpuPreviewWidget::triggerRenderFrame);
    renderTimer->start(16);
}

GpuPreviewWidget::~GpuPreviewWidget() {
    renderTimer->stop();
    m_thread->quit();
    m_thread->wait();
    delete m_worker;
}

void GpuPreviewWidget::ensureInitialized() {
    if (m_initialized || !isVisible())
        return;

    int w = width() * devicePixelRatio();
    int h = height() * devicePixelRatio();
    if (w == 0 || h == 0)
        return;

    m_initialized = true; // 楽観的にtrue、失敗したらonInitFailedで戻す
    emit requestInit(w, h);
}

void GpuPreviewWidget::showEvent(QShowEvent *event) {
    QWidget::showEvent(event);
    ensureInitialized();
}

void GpuPreviewWidget::resizeEvent(QResizeEvent *event) {
    QWidget::resizeEvent(event);
    if (!m_initialized) {
        ensureInitialized();
        return;
    }
    int w = width() * devicePixelRatio();
    int h = height() * devicePixelRatio();
    if (w == 0 || h == 0)
        return;
    emit requestResize(w, h);
}

void GpuPreviewWidget::triggerRenderFrame() {
    ensureInitialized();
    if (!m_initialized)
        return;

    auto project = windowState->network->getProject();
    auto focusedTimelineWidget = windowState->focusedTimeline;
    if (!project.isValid() || !focusedTimelineWidget)
        return;

    Timeline timeline = project.timelineOf(focusedTimelineWidget->timelineIdx);
    CameraInfo *camera = windowState->camera;
    int64_t currentFrame = focusedTimelineWidget->playhead;

    emit requestRender(timeline, camera, currentFrame);
}

void GpuPreviewWidget::paintEvent(QPaintEvent *event) {
    Q_UNUSED(event);
    QPainter painter(this);
    if (!m_currentFrame.isNull()) {
        painter.drawImage(rect(), m_currentFrame);
    } else {
        painter.fillRect(rect(), Qt::black);
    }
}

void GpuPreviewWidget::onFrameReady(QImage img) {
    m_currentFrame = img;
    update(); // paintEventをスケジュール
}

void GpuPreviewWidget::onInitFailed(QString reason) {
    qWarning() << "WgpuPreviewWidget: init failed:" << reason;
    m_initialized = false;
}

void GpuPreviewWidget::onFrameFailed(QString reason) {
    qWarning() << "WgpuPreviewWidget: frame failed:" << reason;
    // m_initializedはいじらない。単発失敗は次フレームで自然にリトライ
}