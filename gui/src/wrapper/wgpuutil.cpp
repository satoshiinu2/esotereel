#include "wgpuutil.h"
#include "../util.h"
#include "esotereel_gui_helper.h"
#include "exception.h"
#include "project/timeline.h"
#include "stringview.h"
#include <QMatrix4x4>
#include <stdexcept>

using NativeWindowHandle = esotereel_gui_helper::NativeWindowHandle;

namespace {
// アプリ側の NativeWindowHandle (util.h) を
// cbindgenが生成した生のFFI型 (esotereel_gui_helper::NativeWindowHandle) に変換する。
// この変換をここ一箇所に閉じ込めることで、util.h側は「Rustのrepr(C)に何を使っているか」を
// 知らなくてよくなる。
NativeWindowHandle toRawHandle(const NativeWindowHandle &handle) {
    NativeWindowHandle raw{};
    raw.kind = handle.kind;
    raw.window_ptr = handle.window_ptr;
    raw.display_ptr = handle.display_ptr;
    return raw;
}
} // namespace

WGpuUtil::WGpuUtil(ClientNetworkHandler *network, const NativeWindowHandle &handle, uint32_t width, uint32_t height)
    : network(*network) {
    auto result = esotereel_gui_helper::wgpuutil_init_surface(toRawHandle(handle), width, height, &wgpuutil_ptr);
    if (!checkWrapperResult(result)) {
        wgpuutil_ptr = nullptr;
    }
}

WGpuUtil::~WGpuUtil() {
    if (wgpuutil_ptr) {
        esotereel_gui_helper::wgpuutil_drop(wgpuutil_ptr);
        wgpuutil_ptr = nullptr;
    }
}

WGpuUtil::WGpuUtil(WGpuUtil &&other) noexcept : wgpuutil_ptr(other.wgpuutil_ptr), network(other.network) {
    other.wgpuutil_ptr = nullptr;
}

WGpuUtil &WGpuUtil::operator=(WGpuUtil &&other) noexcept {
    if (this != &other) {
        if (wgpuutil_ptr) {
            esotereel_gui_helper::wgpuutil_drop(wgpuutil_ptr);
        }
        wgpuutil_ptr = other.wgpuutil_ptr;
        other.wgpuutil_ptr = nullptr;
    }
    return *this;
}

bool WGpuUtil::isValid() const {
    return wgpuutil_ptr != nullptr;
}

void WGpuUtil::renderFrame(Timeline &timeline, CameraInfo *camera, uint64_t currentFrame) {
    if (!isValid())
        return;

    auto result = esotereel_gui_helper::wgpuutil_render_frame(wgpuutil_ptr, network, timeline, camera, currentFrame);

    if (!checkWrapperResult(result)) {
    }
}

void WGpuUtil::attachSurface(const NativeWindowHandle &handle) {
    if (!isValid())
        return;

    auto result = esotereel_gui_helper::wgpuutil_attach_surface(wgpuutil_ptr, toRawHandle(handle));
    if (!checkWrapperResult(result)) {
    }
}

void WGpuUtil::detachSurface() {
    if (!isValid())
        return;

    auto result = esotereel_gui_helper::wgpuutil_detach_surface(wgpuutil_ptr);
    if (!checkWrapperResult(result)) {
    }
}

void WGpuUtil::updateSize(uint32_t width, uint32_t height) {
    if (!isValid())
        return;

    auto result = esotereel_gui_helper::wgpuutil_update_size(wgpuutil_ptr, width, height);
    if (!checkWrapperResult(result)) {
    }
}
