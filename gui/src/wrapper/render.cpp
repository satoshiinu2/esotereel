#include "render.h"
#include "../util.h"
#include "project/timeline.h"
#include "stringview.h"
#include <stdexcept>

WGpuUtil::WGpuUtil(ClientNetworkHandler *network, WId winId, void *display, uint32_t width, uint32_t height)
    : network(*network) {
    this->isWayland = getLinuxDisplayType() == LinuxDisplayType::WAYLAND;
    auto result =
        esotereel_gui_helper::wgpuutil_init_surface((void *)winId, display, width, height, this->isWayland, &raw_ptr);
    if (!StringView::isZero(result)) {
        raw_ptr = nullptr;
        throw std::runtime_error(StringView::toStdString(result));
    }
}

WGpuUtil::~WGpuUtil() {
    if (raw_ptr) {
        esotereel_gui_helper::wgpuutil_drop(raw_ptr);
        raw_ptr = nullptr;
    }
}

WGpuUtil::WGpuUtil(WGpuUtil &&other) noexcept : raw_ptr(other.raw_ptr), network(other.network) {
    other.raw_ptr = nullptr;
}

WGpuUtil &WGpuUtil::operator=(WGpuUtil &&other) noexcept {
    if (this != &other) {
        if (raw_ptr) {
            esotereel_gui_helper::wgpuutil_drop(raw_ptr);
        }
        raw_ptr = other.raw_ptr;
        other.raw_ptr = nullptr;
    }
    return *this;
}

bool WGpuUtil::isValid() const {
    return raw_ptr != nullptr;
}

void WGpuUtil::renderFrame(Timeline &timeline, uint64_t currentFrame) {
    if (!isValid())
        return;

    auto result = esotereel_gui_helper::wgpuutil_render_frame(raw_ptr, network, timeline.raw_ptr, currentFrame);
    if (!StringView::isZero(result)) {
        throw std::runtime_error(StringView::toStdString(result));
    }
}

void WGpuUtil::updateSurface(WId winId, void *display) {
    if (!isValid())
        return;

    auto result = esotereel_gui_helper::wgpuutil_update_surface(raw_ptr, (void *)winId, display, this->isWayland);
    if (!StringView::isZero(result)) {
        throw std::runtime_error(StringView::toStdString(result));
    }
}

void WGpuUtil::updateSize(uint32_t width, uint32_t height) {
    if (!isValid())
        return;

    auto result = esotereel_gui_helper::wgpuutil_update_size(raw_ptr, width, height);
    if (!StringView::isZero(result)) {
        throw std::runtime_error(StringView::toStdString(result));
    }
}