#pragma once

#include "../../util.h"
#include "../main.h"
#include "render_worker.h"
#include <QPlatformSurfaceEvent>
#include <QResizeEvent>
#include <QThread>
#include <QTimer>
#include <QWidget>
#include <QWindow>

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
    void requestRender(Timeline timeline, CameraInfo *camera, int64_t currentFrame);

  private slots:
    void onFrameReady(QImage img);
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
};