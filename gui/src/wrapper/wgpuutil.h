#pragma once

#include "esotereel_gui_helper.h"
#include "network.h"
#include "project/forwards.h"
#include <QMatrix4x4>
#include <cstdint>
#include <qwindowdefs.h>

namespace esotereel_gui_helper {
struct _WGpuUtil;
struct _CameraInfo;
} // namespace esotereel_gui_helper

using RawWGpuUtil = esotereel_gui_helper::_WGpuUtil;
using CameraInfo = esotereel_gui_helper::_CameraInfo;

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
    void renderFrame(Timeline &timeline, CameraInfo &camerainfo, uint64_t currentFrame);
    void updateSurface(WId winId, void *display);
    void updateSize(uint32_t width, uint32_t height);
};