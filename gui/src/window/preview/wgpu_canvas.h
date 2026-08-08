#pragma once

#include "../../util.h"
#include "../../wrapper/wgpuutil.h"
#include "../main.h"
#include "render_worker.h"
#include <QPlatformSurfaceEvent>
#include <QResizeEvent>
#include <QThread>
#include <QTimer>
#include <QWidget>
#include <QWindow>

class WgpuRenderWindow : public QWindow {
    Q_OBJECT
  public:
    explicit WgpuRenderWindow(WindowGState *windowState, QWindow *parent = nullptr);
    ~WgpuRenderWindow() override;

  signals:
    void requestInit(NativeWindowHandle handle, int w, int h);
    void requestResize(int w, int h);
    void requestSurfaceUpdate(NativeWindowHandle handle);
    void requestSurfaceDestroy();
    void requestRender(Timeline timeline, CameraInfo *camera, int64_t currentFrame);

  protected:
    void exposeEvent(QExposeEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;
    bool event(QEvent *ev) override;

  private:
    void ensureInitialized();
    void renderFrame();
    void onInitFailed(QString reason);
    void onFrameFailed(QString reason);

    WindowGState *windowState;
    QThread *m_thread;
    WgpuRenderWorker *m_worker;
    QTimer *renderTimer;
    bool m_initialized = false;
};