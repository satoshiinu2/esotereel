#include "../../wrapper/project/timeline.h"
#include "render_worker.h"

RenderWorker::RenderWorker(WindowGState *state) : windowState(state) {}

void RenderWorker::render(Timeline timeline, CameraInfo *camera, int64_t frame) {
    if (!wgpuutil.has_value()) {
        return;
    }

    wgpuutil->renderFrame(timeline, camera, frame);
}