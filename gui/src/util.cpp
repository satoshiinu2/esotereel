#include "util.h"
#include "esotereel_gui_helper.h"

#if defined(Q_OS_WIN)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#elif defined(Q_OS_LINUX)
#include <qguiapplication_platform.h>
#include <qnativeinterface.h>
#include <qpa/qplatformnativeinterface.h>
#include <xcb/xcb.h>
#elif defined(Q_OS_MACOS)
#include <qnativeinterface.h>
#endif

using NativeWindowHandle = esotereel_gui_helper::NativeWindowHandle;
using PlatformKind = esotereel_gui_helper::PlatformKind;

LinuxDisplayType getLinuxDisplayType() {
    QString platform = QGuiApplication::platformName();

    if (platform == "wayland") {
        return LinuxDisplayType::WAYLAND;
    }
    if (platform == "xcb") {
        return LinuxDisplayType::XCB;
    }
    return LinuxDisplayType::UNKNOWN;
};

NativeWindowHandle getNativeWindowHandle(QWindow *windowhandle) {
    NativeWindowHandle handle{};
    handle.kind = PlatformKind::Unknown;
    handle.window_ptr = nullptr;
    handle.display_ptr = nullptr;

#if defined(Q_OS_LINUX)
    switch (getLinuxDisplayType()) {
    case LinuxDisplayType::XCB: {
        auto *ni = QGuiApplication::platformNativeInterface();

        handle.kind = PlatformKind::Xcb;
        // XCBではwinIdがそのままxcb_window_tになる
        handle.window_ptr = reinterpret_cast<void *>(static_cast<uintptr_t>(windowhandle->winId()));
        handle.display_ptr = static_cast<xcb_connection_t *>(ni->nativeResourceForWindow("connection", windowhandle));

        if (!handle.display_ptr) {
            qWarning() << "could not get xcb_connection_t for window";
        }
        return handle;
    }
    case LinuxDisplayType::WAYLAND: {
        auto *ni = QGuiApplication::platformNativeInterface();
        auto *waylandApp = qGuiApp->nativeInterface<QNativeInterface::QWaylandApplication>();

        handle.kind = PlatformKind::Wayland;
        handle.window_ptr = ni->nativeResourceForWindow("surface", windowhandle);
        handle.display_ptr = waylandApp ? waylandApp->display() : nullptr;

        if (!handle.window_ptr) {
            qWarning() << "could not get wl_surface for window";
        }
        if (!handle.display_ptr) {
            qWarning() << "could not get wl_display";
        }
        return handle;
    }
    case LinuxDisplayType::UNKNOWN:
        qWarning() << "unknown linux display type (neither xcb nor wayland)";
        break;
    }

#elif defined(Q_OS_WIN)
    handle.kind = PlatformKind::Win32;
    handle.window_ptr = reinterpret_cast<void *>(windowhandle->winId());
    handle.display_ptr = GetModuleHandle(NULL);
    return handle;

#elif defined(Q_OS_MACOS)
    handle.kind = PlatformKind::AppKit;
    handle.window_ptr = reinterpret_cast<void *>(windowhandle->winId());
    handle.display_ptr = nullptr;
    return handle;
#endif

    return handle;
}