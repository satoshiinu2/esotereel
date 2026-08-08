#include "render_worker.h"
#include "../../wrapper/exception.h"
#include "../../wrapper/network.h"
#include "../../wrapper/project/timeline.h"

void GpuRenderWorker::initialize(int w, int h) {
    if (wgpuutil_ptr || w == 0 || h == 0)
        return;

    // Rust側 wgpuutil_new(width, height) -> *mut WGpuUtil のFFI呼び出し
    auto result = esotereel_gui_helper::wgpuutil_new(w, h, &wgpuutil_ptr);
    if (!checkWrapperResult(result)) {
        wgpuutil_ptr = nullptr;
        emit initFailed("failed to init wgpuutil");
        return;
    }

    // OffscreenTargetも同時に作る
    auto result2 = esotereel_gui_helper::offscreen_target_new(wgpuutil_ptr, w, h, &offscreen_ptr);
    if (!checkWrapperResult(result2)) {
        offscreen_ptr = nullptr;
        emit initFailed("failed to init offscreen target");
    }
}

void GpuRenderWorker::resize(int w, int h) {
    if (!wgpuutil_ptr || w == 0 || h == 0)
        return;

    // OffscreenTargetを作り直す(サイズ変更のたびに再生成)
    if (offscreen_ptr) {
        esotereel_gui_helper::offscreen_target_drop(offscreen_ptr);
        offscreen_ptr = nullptr;
    }
    auto result = esotereel_gui_helper::offscreen_target_new(wgpuutil_ptr, w, h, &offscreen_ptr);
    if (!checkWrapperResult(result)) {
        offscreen_ptr = nullptr;
        emit frameFailed("failed to resize offscreen target");
    }
}

void GpuRenderWorker::renderFrame(Timeline timeline, CameraInfo *camera, int64_t currentFrame) {
    if (!wgpuutil_ptr || !offscreen_ptr || busy)
        return;
    busy = true;

    uint8_t *data = nullptr;
    size_t len = 0;
    uint32_t width = 0, height = 0;
    const esotereel_gui_helper::ClientNetworkHandler *raw_network = *windowState->network;

    auto result = esotereel_gui_helper::wgpuutil_render_frame_offscreen(
        wgpuutil_ptr, offscreen_ptr, raw_network, timeline, camera, currentFrame, &data, &len, &width, &height);

    if (checkWrapperResult(result) && data) {
        QImage img(data, width, height, width * 4, QImage::Format_RGBA8888);
        emit frameReady(img.copy()); // copy()でQt管理のバッファに複製
        esotereel_gui_helper::wgpuutil_free_buffer(data, len);
    } else {
        emit frameFailed("render failed");
    }

    busy = false;
}

GpuRenderWorker::~GpuRenderWorker() {
    if (offscreen_ptr) {
        esotereel_gui_helper::offscreen_target_drop(offscreen_ptr);
    }
    if (wgpuutil_ptr) {
        esotereel_gui_helper::wgpuutil_drop(wgpuutil_ptr);
    }
}