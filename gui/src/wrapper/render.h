#pragma once

#include "../util.h"
#include "esotereel_gui_helper.h"
#include "project/timeline.h"
#include <QWidget>
#include <qguiapplication_platform.h>
#include <qlogging.h>
#include <qwindowdefs.h>
#include <sys/types.h>

using RawWGpuUtil = esotereel_gui_helper::_WGpuUtil;

class WGpuUtil {

  private:
    RawWGpuUtil *raw_ptr;
    bool isWayland;

  public:
    WGpuUtil(WId winId, void *display, uint32_t width, uint32_t height) {
        // qDebug() << QGuiApplication::platformName();
        this->isWayland = getLinuxDisplayType() == LinuxDisplayType::WAYLAND;
        raw_ptr = esotereel_gui_helper::wgpuutil_init_surface((void *)winId, display, width, height, this->isWayland);
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
    WGpuUtil(WGpuUtil &&other) noexcept : raw_ptr(other.raw_ptr) {
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
        esotereel_gui_helper::render_frame(raw_ptr, timeline.raw_ptr, currentFrame);
    }

    void updateSurface(WId winId, void *display) {
        esotereel_gui_helper::wgpuutil_update_surface(raw_ptr, (void *)winId, display, this->isWayland);
    }

    void updateSize(uint32_t width, uint32_t height) {
        esotereel_gui_helper::wgpuutil_update_size(raw_ptr, width, height);
    }
};