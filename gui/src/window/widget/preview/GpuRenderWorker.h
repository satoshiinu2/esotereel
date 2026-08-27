#pragma once

#include "esotereel_gui_helper.h"
#include "window/MainWindow.h"
#include "window/widget/timeline/TimelineWidget.h"
#include "wrapper/project/Timeline.h"
#include "wrapper/project/camera.h"
#include <QObject>
#include <QThread>

namespace esotereel::window {
using WGpuUtil = esotereel_gui_helper::WGpuUtil;
using OffscreenTarget = esotereel_gui_helper::OffscreenTarget;
class GpuRenderWorker : public QObject {
    Q_OBJECT
  public:
    explicit GpuRenderWorker(WindowGState *windowState) : windowState(windowState) {}
    ~GpuRenderWorker();

  public slots:
    void initialize(int w, int h);
    void resize(int w, int h);
    void renderFrame(Timeline timeline, CameraInfo *camera, int64_t currentFrame);

  signals:
    void frameReady(QImage img);
    void initFailed(QString reason);
    void frameFailed(QString reason);

  private:
    WindowGState *windowState;
    WGpuUtil *wgpuutil_ptr = nullptr;
    OffscreenTarget *offscreen_ptr = nullptr;
    bool busy = false;
};
} // namespace esotereel::window