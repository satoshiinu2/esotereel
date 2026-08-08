#include "util.h"
#include "esotereel_gui_helper.h"
#include <QWidget>
#include <QWindow>

#if defined(Q_OS_WIN)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#elif defined(Q_OS_LINUX)
#include <qguiapplication_platform.h>
#include <qnativeinterface.h>
#include <qpa/qplatformnativeinterface.h>
#include <wayland-client.h>
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
}

NativeWindowHandle getNativeWindowHandle(QWindow *window) {
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
        handle.window_ptr = reinterpret_cast<void *>(static_cast<uintptr_t>(window->winId()));
        handle.display_ptr = static_cast<xcb_connection_t *>(ni->nativeResourceForWindow("connection", window));

        if (!handle.display_ptr) {
            qWarning() << "could not get xcb_connection_t for window";
        }
        return handle;
    }
    case LinuxDisplayType::WAYLAND: {
        auto *ni = QGuiApplication::platformNativeInterface();
        auto *waylandApp = qGuiApp->nativeInterface<QNativeInterface::QWaylandApplication>();

        handle.kind = PlatformKind::Wayland;
        handle.window_ptr = ni->nativeResourceForWindow("surface", window);
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

NativeWindowHandle getNativeWindowHandle(QWidget *widget) {
    if (!widget) {
        return NativeWindowHandle{PlatformKind::Unknown, nullptr, nullptr};
    }

    // 1. まず QWidget 自身または親から QWindow を取得してみる
    QWindow *window = widget->windowHandle();

    // 2. もし windowHandle() が nullptr の場合（まだウィンドウが表示されていない等）
    // widget->winId() を呼ぶことで強制的に QWindow / HWND / wl_surface 等を生成（Ensure Created）させる
    if (!window) {
        widget->winId();
        window = widget->windowHandle();
    }

    // 3. トップレベルウィンドウの handle が必要な場合は topLevelWidget を経由させることも可能ですが、
    // 子ウィンドウとして独立させる場合はそのまま widget の windowHandle() または winId() を使います。
    if (window) {
        return getNativeWindowHandle(window); // 既存の QWindow* 版を呼び出す
    }

    // 万が一 windowHandle() が取得できなかった場合のフォールバック (Linux Wayland等での直接取得)
    NativeWindowHandle handle{};
    handle.kind = PlatformKind::Unknown;

#if defined(Q_OS_LINUX)
    switch (getLinuxDisplayType()) {
    case LinuxDisplayType::XCB: {
        auto *ni = QGuiApplication::platformNativeInterface();
        handle.kind = PlatformKind::Xcb;
        handle.window_ptr = reinterpret_cast<void *>(static_cast<uintptr_t>(widget->winId()));
        handle.display_ptr = ni ? ni->nativeResourceForWindow("connection", nullptr) : nullptr;
        return handle;
    }
    case LinuxDisplayType::WAYLAND: {
        auto *ni = QGuiApplication::platformNativeInterface();
        auto *waylandApp = qGuiApp->nativeInterface<QNativeInterface::QWaylandApplication>();
        handle.kind = PlatformKind::Wayland;
        // QWidget の winId() 経由で nativeResource を取る場合
        handle.window_ptr = ni ? ni->nativeResourceForWindow("surface", widget->windowHandle()) : nullptr;
        handle.display_ptr = waylandApp ? waylandApp->display() : nullptr;
        return handle;
    }
    default:
        break;
    }
#elif defined(Q_OS_WIN)
    handle.kind = PlatformKind::Win32;
    handle.window_ptr = reinterpret_cast<void *>(widget->winId());
    handle.display_ptr = GetModuleHandle(NULL);
    return handle;
#elif defined(Q_OS_MACOS)
    handle.kind = PlatformKind::AppKit;
    handle.window_ptr = reinterpret_cast<void *>(widget->winId());
    handle.display_ptr = nullptr;
    return handle;
#endif

    return handle;
}

void forceDesyncSubsurface(QWindow *window) {
    auto *ni = QGuiApplication::platformNativeInterface();

#if defined(Q_OS_LINUX)
    void *subsurfacePtr = ni->nativeResourceForWindow("subsurface", window);
    if (!subsurfacePtr) {
        qWarning() << "could not get wl_subsurface for window";
        return;
    }

    auto *subsurface = static_cast<wl_subsurface *>(subsurfacePtr);
    wl_subsurface_set_desync(subsurface);
#endif
}