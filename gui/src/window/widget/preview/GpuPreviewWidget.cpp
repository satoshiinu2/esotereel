#include "GpuPreviewWidget.h"
#include "GpuRenderWorker.h"
#include "ffi/ClientState.h"
#include "ffi/Result.h"
#include "ffi/project/Project.h"
#include "ffi/project/Timeline.h"
#include "window/MainWindow.h"
#include "window/widget/timeline/TimelineCanvasWidget.h"
#include "window/widget/timeline/TimelineWidget.h"
#include <QDebug>
#include <QMetaObject>
#include <QPainter>
#include <QThread>
#include <cmath>

namespace esotereel::window {
namespace {
constexpr int kRenderIntervalMs = 16;
constexpr qint64 kInitRetryIntervalMs = 1000; // 初期化失敗後、再試行までの待ち時間
} // namespace

GpuPreviewWidget::GpuPreviewWidget(WindowGState *windowState, QWidget *parent)
    : QWidget(parent), windowState(windowState) {
    m_thread = new QThread(this);
    m_worker = new GpuRenderWorker(windowState); // moveToThreadするので親は付けない
    m_worker->moveToThread(m_thread);
    connect(m_thread, &QThread::finished, m_worker, &QObject::deleteLater);
    m_thread->start();

    connect(this, &GpuPreviewWidget::requestInit, m_worker, &GpuRenderWorker::initialize, Qt::QueuedConnection);
    connect(this, &GpuPreviewWidget::requestResize, m_worker, &GpuRenderWorker::resize, Qt::QueuedConnection);
    connect(this, &GpuPreviewWidget::requestRender, m_worker, &GpuRenderWorker::renderFrame, Qt::QueuedConnection);
    connect(m_worker, &GpuRenderWorker::frameReady, this, &GpuPreviewWidget::onFrameReady, Qt::QueuedConnection);
    connect(m_worker, &GpuRenderWorker::renderFinished, this, &GpuPreviewWidget::onRenderFinished,
            Qt::QueuedConnection);
    connect(m_worker, &GpuRenderWorker::initFailed, this, &GpuPreviewWidget::onInitFailed, Qt::QueuedConnection);
    connect(m_worker, &GpuRenderWorker::frameFailed, this, &GpuPreviewWidget::onFrameFailed, Qt::QueuedConnection);

    renderTimer = new QTimer(this);
    connect(renderTimer, &QTimer::timeout, this, &GpuPreviewWidget::triggerRenderFrame);
    renderTimer->start(kRenderIntervalMs);
}

GpuPreviewWidget::~GpuPreviewWidget() {
    renderTimer->stop();

    // 破棄中のウィジェットにワーカーからのシグナルが届かないよう双方向に切断
    disconnect(m_worker, nullptr, this, nullptr);
    disconnect(this, nullptr, m_worker, nullptr);

    // キュー済みの処理が終わった後、ワーカースレッド上でwgpuリソースを確実に解放する
    QMetaObject::invokeMethod(m_worker, &GpuRenderWorker::shutdown, Qt::BlockingQueuedConnection);

    m_thread->quit();
    m_thread->wait(); // finished -> m_worker->deleteLater() がワーカースレッド側で処理される
}

void GpuPreviewWidget::ensureInitialized() {
    if (m_initialized || !isVisible())
        return;

    // 失敗直後に16ms周期で再試行し続けないようにする
    if (m_initFailTimer.isValid() && !m_initFailTimer.hasExpired(kInitRetryIntervalMs))
        return;

    const qreal dpr = devicePixelRatioF();
    const int w = qRound(width() * dpr);
    const int h = qRound(height() * dpr);
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
    const qreal dpr = devicePixelRatioF();
    const int w = qRound(width() * dpr);
    const int h = qRound(height() * dpr);
    if (w == 0 || h == 0)
        return;
    emit requestResize(w, h);
}

void GpuPreviewWidget::triggerRenderFrame() {
    if (!isVisible())
        return; // 非表示中は描画しない

    ensureInitialized();
    if (!m_initialized) {
        return;
    }

    // ワーカーが前のフレームを処理中ならスキップ(キューに溜めない)
    if (m_renderInFlight) {
        return;
    }

    auto projectResult = windowState->state->getProject();
    if (projectResult.isError()) {
        return;
    }
    auto project = std::move(projectResult.unwrap());
    auto focusedTimelineWidget = windowState->focusedTimeline;
    if (!project.isValid() || !focusedTimelineWidget) {
        return;
    }

    TimelineCanvasWidget *canvas = focusedTimelineWidget->canvas;
    if (!canvas || !windowState->camera) {
        return;
    }

    // カメラは値コピーして渡す。ポインタ渡しだとワーカー実行時に
    // 指す先が無効/書き換え中になっている可能性がある。
    const CameraInfo camera = *windowState->camera;
    const int64_t currentFrame = canvas->playhead;

    m_renderInFlight = true;
    emit requestRender(canvas->timelineId, camera, currentFrame);
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

void GpuPreviewWidget::onRenderFinished() {
    m_renderInFlight = false;
}

void GpuPreviewWidget::onInitFailed(QString reason) {
    qWarning() << "GpuPreviewWidget: init failed:" << reason;
    m_initialized = false;
    m_initFailTimer.start(); // この時刻から kInitRetryIntervalMs 後に再試行
}

void GpuPreviewWidget::onFrameFailed(QString reason) {
    qWarning() << "GpuPreviewWidget: frame failed:" << reason;
    // m_initializedはいじらない。単発失敗は次フレームで自然にリトライ
}
} // namespace esotereel::window
