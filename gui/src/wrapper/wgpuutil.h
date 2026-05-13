#pragma once

#include "network.h"
#include "project/forwards.h"
#include <cstdint>
#include <qwindowdefs.h>

namespace esotereel_gui_helper {
struct _WGpuUtil;
}

using RawWGpuUtil = esotereel_gui_helper::_WGpuUtil;

class WGpuUtil {

  private:
    RawWGpuUtil *wgpuutil_ptr;
    bool isWayland;
    ClientNetworkHandler &network;

  public:
    WGpuUtil(ClientNetworkHandler *network, WId winId, void *display, uint32_t width, uint32_t height);
    ~WGpuUtil();

    WGpuUtil(const WGpuUtil &) = delete;
    WGpuUtil &operator=(const WGpuUtil &) = delete;

    WGpuUtil(WGpuUtil &&other) noexcept;
    WGpuUtil &operator=(WGpuUtil &&other) noexcept;

    bool isValid() const;
    void renderFrame(Timeline &timeline, uint64_t currentFrame);
    void updateSurface(WId winId, void *display);
    void updateSize(uint32_t width, uint32_t height);
};