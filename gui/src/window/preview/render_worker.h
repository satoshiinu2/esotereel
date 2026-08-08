#pragma once

#include "../../wrapper/project/camera.fwd.h"
#include "../../wrapper/project/timeline.h"
#include "../main.h"
#include "../timeline/timeline.h"
#include "esotereel_gui_helper.h"
#include <QObject>
#include <QThread>

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