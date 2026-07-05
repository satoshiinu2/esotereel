#pragma once

#include "../util.h"
#include "network.h"
#include "project/camera.fwd.h"
#include "project/forwards.h"
#include <QMatrix4x4>
#include <cstdint>

namespace esotereel_gui_helper {
struct WGpuUtil;
struct CameraInfo;
} // namespace esotereel_gui_helper

using RawWGpuUtil = esotereel_gui_helper::WGpuUtil;
using CameraInfo = esotereel_gui_helper::CameraInfo;
using NativeWindowHandle = esotereel_gui_helper::NativeWindowHandle;

class WGpuUtil {

  private:
    RawWGpuUtil *wgpuutil_ptr;
    ClientNetworkHandler &network;

  public:
    // handle は util.h の getNativeWindowHandle() が返すアプリ側の型。
    // Rust/cbindgen生成の生の型への変換はwgpuutil.cpp内で行う。
    WGpuUtil(ClientNetworkHandler *network, const NativeWindowHandle &handle, uint32_t width, uint32_t height);
    ~WGpuUtil();

    WGpuUtil(const WGpuUtil &) = delete;
    WGpuUtil &operator=(const WGpuUtil &) = delete;

    WGpuUtil(WGpuUtil &&other) noexcept;
    WGpuUtil &operator=(WGpuUtil &&other) noexcept;

    bool isValid() const;
    void renderFrame(Timeline &timeline, CameraInfo *camera, uint64_t currentFrame);
    void updateSurface(const NativeWindowHandle &handle);
    void updateSize(uint32_t width, uint32_t height);
};
