#pragma once

#include "GpuRenderWorker.h"
#include "window/MainWindow.h"
#include <QElapsedTimer>
#include <QImage>
#include <QResizeEvent>
#include <QShowEvent>
#include <QThread>
#include <QTimer>
#include <QWidget>

namespace esotereel::window {
class GpuPreviewWidget : public QWidget {
    Q_OBJECT
  public:
    explicit GpuPreviewWidget(WindowGState *windowState, QWidget *parent = nullptr);
    ~GpuPreviewWidget() override;

  protected:
    void paintEvent(QPaintEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;
    void showEvent(QShowEvent *event) override;

  signals:
    void requestInit(int w, int h);
    void requestResize(int w, int h);
    void requestRender(TimelineId timelineId, CameraInfo camera, int64_t currentFrame);

  private slots:
    void onFrameReady(QImage img);
    void onRenderFinished();
    void onInitFailed(QString reason);
    void onFrameFailed(QString reason);

  private:
    void ensureInitialized();
    void triggerRenderFrame();

    WindowGState *windowState;
    QThread *m_thread;
    GpuRenderWorker *m_worker;
    QTimer *renderTimer;
    QImage m_currentFrame;
    bool m_initialized = false;
    bool m_renderInFlight = false;  // GUIスレッドでのみ読み書きする
    QElapsedTimer m_initFailTimer;  // 初期化失敗後のリトライ間隔制御
};
} // namespace esotereel::window
