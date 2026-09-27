#include "GpuRenderWorker.h"
#include "ffi/Array.h"
#include "ffi/ClientState.h"
#include "ffi/Result.h"
#include "ffi/WrapperResult.h"
#include "ffi/project/Timeline.h"

namespace esotereel::window {

using FrameRenderResult = esotereel_gui_helper::FrameRenderResult;

void GpuRenderWorker::initialize(int w, int h) {
    if (wgpuutil_ptr || w == 0 || h == 0)
        return;

    // Rust側 wgpuutil_new(width, height) -> *mut WGpuUtil のFFI呼び出し
    auto result = Result(esotereel_gui_helper::wgpuutil_new(w, h));
    if (!result.isOk()) {
        wgpuutil_ptr = nullptr;
        emit initFailed("failed to init wgpuutil");
        return;
    }

    wgpuutil_ptr = result.unwrap();

    // OffscreenTargetも同時に作る
    auto result2 = Result(esotereel_gui_helper::offscreen_target_new(wgpuutil_ptr, w, h));
    if (!result2.isOk()) {
        offscreen_ptr = nullptr;
        emit initFailed("failed to init offscreen target");
        return;
    }

    offscreen_ptr = result2.unwrap();
}

void GpuRenderWorker::resize(int w, int h) {
    if (!wgpuutil_ptr || w == 0 || h == 0)
        return;

    // OffscreenTargetを作り直す(サイズ変更のたびに再生成)
    if (offscreen_ptr) {
        esotereel_gui_helper::offscreen_target_drop(offscreen_ptr);
        offscreen_ptr = nullptr;
    }
    auto result = Result(esotereel_gui_helper::offscreen_target_new(wgpuutil_ptr, w, h));
    if (!result.isOk()) {
        offscreen_ptr = nullptr;
        emit frameFailed("failed to resize offscreen target");
        return;
    }

    offscreen_ptr = result.unwrap();
}

void GpuRenderWorker::renderFrame(TimelineId timelineId, CameraInfo *camera, int64_t currentFrame) {
    if (!wgpuutil_ptr || !offscreen_ptr || busy)
        return;
    busy = true;

    const esotereel_gui_helper::ClientStateHandle *raw_network = *windowState->state;

    auto result = Result(esotereel_gui_helper::wgpuutil_render_frame_offscreen(wgpuutil_ptr, offscreen_ptr, raw_network,
                                                                               camera, timelineId, currentFrame));

    if (result.isOk()) {
        auto unwrap = result.unwrap();
        auto frameData = ArrayFreeGuard(unwrap.data);
        QImage img(frameData.data(), unwrap.width, unwrap.height, unwrap.width * 4, QImage::Format_RGBA8888);
        emit frameReady(img.copy()); // copy()でQt管理のバッファに複製
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
} // namespace esotereel::window