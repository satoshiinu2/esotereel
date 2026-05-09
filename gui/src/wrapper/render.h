#pragma once

#include "../util.h"
#include "esotereel_gui_helper.h"
#include "network.h"
#include "project/timeline.h"
#include "stringview.h"
#include <QWidget>
#include <qguiapplication_platform.h>
#include <qlogging.h>
#include <qwindowdefs.h>
#include <stdexcept>
#include <sys/types.h>

using RawWGpuUtil = esotereel_gui_helper::_WGpuUtil;

class WGpuUtil {

  private:
    RawWGpuUtil *raw_ptr;
    bool isWayland;
    ClientNetworkHandler &network;

  public:
    WGpuUtil(ClientNetworkHandler *network, WId winId, void *display, uint32_t width, uint32_t height) : network(*network) {
        // qDebug() << QGuiApplication::platformName();
        this->isWayland = getLinuxDisplayType() == LinuxDisplayType::WAYLAND;
        auto result = esotereel_gui_helper::wgpuutil_init_surface((void *)winId, display, width, height, this->isWayland, &raw_ptr);
        if (!StringView::isZero(result)) {
            raw_ptr = nullptr;
            throw std::runtime_error(StringView::toStdString(result));
        }
    }
    ~WGpuUtil() {
        if (raw_ptr) {
            esotereel_gui_helper::wgpuutil_drop(raw_ptr);
            raw_ptr = nullptr;
        }
    }
    WGpuUtil(const WGpuUtil &) = delete;
    WGpuUtil &operator=(const WGpuUtil &) = delete;

    //  move
    WGpuUtil(WGpuUtil &&other) noexcept : raw_ptr(other.raw_ptr), network(other.network) {
        other.raw_ptr = nullptr;
    }

    // drop
    WGpuUtil &operator=(WGpuUtil &&other) noexcept {
        if (this != &other) {
            if (raw_ptr) {
                esotereel_gui_helper::wgpuutil_drop(raw_ptr);
            }
            raw_ptr = other.raw_ptr;
            other.raw_ptr = nullptr;
        }
        return *this;
    }

    bool isValid() const { return raw_ptr != nullptr; }
    void renderFrame(Timeline timeline, u_int64_t currentFrame) {
        auto result = esotereel_gui_helper::wgpuutil_render_frame(raw_ptr, network, timeline.raw_ptr, currentFrame);
        if (!StringView::isZero(result)) {
            throw std::runtime_error(StringView::toStdString(result));
        }
    }

    void updateSurface(WId winId, void *display) {
        auto result = esotereel_gui_helper::wgpuutil_update_surface(raw_ptr, (void *)winId, display, this->isWayland);
        if (!StringView::isZero(result)) {
            throw std::runtime_error(StringView::toStdString(result));
        }
    }

    void updateSize(uint32_t width, uint32_t height) {
        auto result = esotereel_gui_helper::wgpuutil_update_size(raw_ptr, width, height);
        if (!StringView::isZero(result)) {
            throw std::runtime_error(StringView::toStdString(result));
        }
    }
};