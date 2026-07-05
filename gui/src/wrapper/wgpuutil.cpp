#include "wgpuutil.h"
#include "../util.h"
#include "esotereel_gui_helper.h"
#include "project/timeline.h"
#include "stringview.h"
#include <QMatrix4x4>
#include <stdexcept>

using NativeWindowHandle = esotereel_gui_helper::NativeWindowHandle;

WGpuUtil::WGpuUtil(ClientNetworkHandler *network, const NativeWindowHandle &handle, uint32_t width, uint32_t height)
    : network(*network) {
    auto result = esotereel_gui_helper::wgpuutil_init_surface(handle, width, height, &wgpuutil_ptr);
    if (!StringView::isZero(result)) {
        wgpuutil_ptr = nullptr;
        throw std::runtime_error(StringView::toStdString(result));
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

    if (!StringView::isZero(result)) {
        throw std::runtime_error(StringView::toStdString(result));
    }
}

void WGpuUtil::updateSurface(const NativeWindowHandle &handle) {
    if (!isValid())
        return;

    auto result = esotereel_gui_helper::wgpuutil_update_surface(wgpuutil_ptr, handle);
    if (!StringView::isZero(result)) {
        throw std::runtime_error(StringView::toStdString(result));
    }
}

void WGpuUtil::updateSize(uint32_t width, uint32_t height) {
    if (!isValid())
        return;

    auto result = esotereel_gui_helper::wgpuutil_update_size(wgpuutil_ptr, width, height);
    if (!StringView::isZero(result)) {
        throw std::runtime_error(StringView::toStdString(result));
    }
}