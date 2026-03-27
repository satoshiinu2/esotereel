#pragma once

#include "nomyoedit_gui_helper.h"
#include <QWidget>
#include <qguiapplication_platform.h>
#include <qwindowdefs.h>

static bool isWayland = QGuiApplication::platformName().startsWith("wayland");
class WWGpuUtil {

  private:
    nomyoedit_gui_helper::WGpuUtil *raw_ptr;

  public:
    WWGpuUtil(WId winId, void *display, uint32_t width, uint32_t height) {
        raw_ptr = nomyoedit_gui_helper::wgpuutil_init_surface((void *)winId, display, width, height, isWayland);
    }
    ~WWGpuUtil() {
        if (raw_ptr) {
            nomyoedit_gui_helper::wgpuutil_drop(raw_ptr);
            raw_ptr = nullptr;
        }
    }
    WWGpuUtil(const WWGpuUtil &) = delete;
    WWGpuUtil &operator=(const WWGpuUtil &) = delete;

    WWGpuUtil(WWGpuUtil &&other) noexcept : raw_ptr(other.raw_ptr) {
        other.raw_ptr = nullptr;
    }
    WWGpuUtil &operator=(WWGpuUtil &&other) noexcept {
        if (this != &other) {
            if (raw_ptr)
                nomyoedit_gui_helper::wgpuutil_drop(raw_ptr);
            raw_ptr = other.raw_ptr;
            other.raw_ptr = nullptr;
        }
        return *this;
    }

    bool isValid() const { return raw_ptr != nullptr; }
    void renderFrame() {
        nomyoedit_gui_helper::render_frame(raw_ptr);
    }

    void updateSurface(WId winId, void *display) {
        nomyoedit_gui_helper::wgpuutil_update_surface(raw_ptr, (void *)winId, display, isWayland);
    }

    void updateSize(uint32_t width, uint32_t height) {
        nomyoedit_gui_helper::wgpuutil_update_size(raw_ptr, width, height);
    }
};