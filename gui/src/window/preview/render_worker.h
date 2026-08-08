#pragma once

#include "../../wrapper/project/camera.fwd.h"
#include "../../wrapper/project/timeline.h"
#include "../../wrapper/wgpuutil.h"
#include "../main.h"
#include "../timeline/timeline.h"
#include <QObject>
#include <QThread>
class WgpuRenderWorker : public QObject {
    Q_OBJECT
  public:
    explicit WgpuRenderWorker(WindowGState *windowState, QObject *parent = nullptr)
        : QObject(parent), windowState(windowState) {}

  public slots:
    void initialize(NativeWindowHandle handle, int w, int h);
    void resize(int w, int h);
    void updateSurface(NativeWindowHandle handle);
    void destroySurface();
    void renderFrame(Timeline timeline, CameraInfo *camera, int64_t currentFrame);

  signals:
    void frameFailed(QString reason);
    void initFailed(QString reason);

  private:
    WindowGState *windowState;
    std::optional<WGpuUtil> wgpuutil;
    bool busy = false;
};