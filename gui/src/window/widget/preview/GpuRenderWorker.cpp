#include "GpuRenderWorker.h"
#include "ffi/Array.h"
#include "ffi/ClientState.h"
#include "ffi/Result.h"
#include "ffi/WrapperResult.h"
#include "ffi/project/Timeline.h"
#include <QScopeGuard>

namespace esotereel::window {

using FrameRenderResult = esotereel_gui_helper::FrameRenderResult;

void GpuRenderWorker::releaseOffscreen() {
    if (offscreen_ptr) {
        esotereel_gui_helper::offscreen_target_drop(offscreen_ptr);
        offscreen_ptr = nullptr;
    }
}

void GpuRenderWorker::releaseWgpu() {
    // OffscreenTargetはWGpuUtilに依存するので先に落とす
    releaseOffscreen();
    if (wgpuutil_ptr) {
        esotereel_gui_helper::wgpuutil_drop(wgpuutil_ptr);
        wgpuutil_ptr = nullptr;
    }
}

void GpuRenderWorker::initialize(int w, int h) {
    if (wgpuutil_ptr || w == 0 || h == 0)
        return;

    // Rust側 wgpuutil_new(width, height) -> *mut WGpuUtil のFFI呼び出し
    auto result = Result(esotereel_gui_helper::wgpuutil_new(w, h));
    if (!result.isOk()) {
        emit initFailed("failed to init wgpuutil");
        return;
    }
    wgpuutil_ptr = result.unwrap();

    // OffscreenTargetも同時に作る
    auto result2 = Result(esotereel_gui_helper::offscreen_target_new(wgpuutil_ptr, w, h));
    if (!result2.isOk()) {
        // 中途半端な状態を残さない。残すと再試行時に「wgpuutil_ptr有り」で
        // 早期returnしてしまい、offscreenが永久に作られなくなる。
        releaseWgpu();
        emit initFailed("failed to init offscreen target");
        return;
    }
    offscreen_ptr = result2.unwrap();
}

void GpuRenderWorker::resize(int w, int h) {
    if (!wgpuutil_ptr || w == 0 || h == 0)
        return;

    // OffscreenTargetを作り直す(サイズ変更のたびに再生成)
    releaseOffscreen();
    auto result = Result(esotereel_gui_helper::offscreen_target_new(wgpuutil_ptr, w, h));
    if (!result.isOk()) {
        emit frameFailed("failed to resize offscreen target");
        return;
    }
    offscreen_ptr = result.unwrap();
}

void GpuRenderWorker::renderFrame(TimelineId timelineId, CameraInfo camera, int64_t currentFrame) {
    // どの経路で抜けても必ず通知する(GUI側がin-flightフラグを下ろせるように)
    const auto finish = qScopeGuard([this] { emit renderFinished(); });

    if (!wgpuutil_ptr || !offscreen_ptr)
        return;

    if (!windowState->state || !windowState->state->isValid()) {
        emit frameFailed("invalid client state");
        return;
    }

    const esotereel_gui_helper::ClientStateHandle *raw_network = *windowState->state;

    auto result = Result(esotereel_gui_helper::wgpuutil_render_frame_offscreen(wgpuutil_ptr, offscreen_ptr, raw_network,
                                                                               &camera, timelineId, currentFrame));

    if (result.isOk()) {
        auto unwrap = result.unwrap();
        // Defensive check: ensure the buffer size matches expected size
        const size_t expected_size = static_cast<size_t>(unwrap.width) * unwrap.height * 4;
        if (unwrap.data.ptr == nullptr || Array::size(unwrap.data) != expected_size) {
            emit frameFailed("invalid frame buffer dimensions");
            return;
        }
        auto frameData = ArrayFreeGuard(unwrap.data);
        QImage img(frameData.data(), unwrap.width, unwrap.height, unwrap.width * 4, QImage::Format_RGBA8888);
        emit frameReady(img.copy()); // copy()でQt管理のバッファに複製
    } else {
        emit frameFailed("render failed");
    }
}

void GpuRenderWorker::shutdown() {
    releaseWgpu();
}

GpuRenderWorker::~GpuRenderWorker() {
    releaseWgpu();
}

} // namespace esotereel::window