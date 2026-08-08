#include "render_worker.h"
#include "../../wrapper/project/timeline.h"

void WgpuRenderWorker::initialize(NativeWindowHandle handle, int w, int h) {
    if (wgpuutil.has_value() || w == 0 || h == 0)
        return;

    try {
        wgpuutil = WGpuUtil(windowState->network, handle, w, h);
    } catch (const std::exception &e) {
        qWarning() << "WgpuRenderWorker: initialize failed:" << e.what();
        wgpuutil.reset();
        emit initFailed(e.what());
    }
}

void WgpuRenderWorker::resize(int w, int h) {
    if (!wgpuutil.has_value() || w == 0 || h == 0)
        return;

    try {
        wgpuutil->updateSize(w, h);
    } catch (const std::exception &e) {
        qWarning() << "WgpuRenderWorker: resize failed:" << e.what();
        emit frameFailed(e.what());
    }
}

void WgpuRenderWorker::updateSurface(NativeWindowHandle handle) {
    if (!wgpuutil.has_value())
        return;

    try {
        wgpuutil->attachSurface(handle);
    } catch (const std::exception &e) {
        qWarning() << "WgpuRenderWorker: updateSurface failed:" << e.what();
        wgpuutil->detachSurface();
        emit initFailed(e.what());
    }
}

void WgpuRenderWorker::destroySurface() {
    if (wgpuutil.has_value()) {
        wgpuutil->detachSurface();
    }
}

void WgpuRenderWorker::renderFrame(Timeline timeline, CameraInfo *camera, int64_t currentFrame) {
    if (!wgpuutil.has_value() || busy)
        return;
    busy = true;
    try {
        wgpuutil->renderFrame(timeline, camera, currentFrame);
    } catch (const std::exception &e) {
        qWarning() << "render failed:" << e.what();
        emit frameFailed(e.what());
    }
    busy = false;
}