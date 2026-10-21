#pragma once

#include "esotereel_gui_helper.h"
#include "window/MainWindow.h"
#include <QImage>
#include <QObject>
#include <QString>

namespace esotereel::window {
using WGpuUtil = esotereel_gui_helper::WGpuUtil;
using OffscreenTarget = esotereel_gui_helper::OffscreenTarget;

class GpuRenderWorker : public QObject {
    Q_OBJECT
  public:
    explicit GpuRenderWorker(WindowGState *windowState) : windowState(windowState) {}
    ~GpuRenderWorker() override;

  public slots:
    void initialize(int w, int h);
    void resize(int w, int h);
    // CameraInfoは値渡し(GUIスレッド側のポインタ先に依存しない)
    void renderFrame(TimelineId timelineId, CameraInfo camera, int64_t currentFrame);
    // wgpuリソースをワーカースレッド上で解放する。冪等。
    void shutdown();

  signals:
    void frameReady(QImage img);
    // renderFrameの成功/失敗/スキップに関わらず必ず1回emitされる(GUI側のin-flight解除用)
    void renderFinished();
    void initFailed(QString reason);
    void frameFailed(QString reason);

  private:
    void releaseOffscreen();
    void releaseWgpu();

    WindowGState *windowState;
    WGpuUtil *wgpuutil_ptr = nullptr;
    OffscreenTarget *offscreen_ptr = nullptr;
};
} // namespace esotereel::window
