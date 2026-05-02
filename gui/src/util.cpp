#include "util.h"

#if defined(Q_OS_WIN)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#elif defined(Q_OS_LINUX)
#include <qguiapplication_platform.h>
#include <qnativeinterface.h>
#include <qpa/qplatformnativeinterface.h>
#include <xcb/xcb.h>
#endif

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

void *getNativeDisplay(QWindow *windowhandle) {
#if defined(Q_OS_LINUX)
    auto dispType = getLinuxDisplayType();
    switch (dispType) {
    case LinuxDisplayType::XCB: {
        //     auto *xcbApp = qGuiApp->nativeInterface<QNativeInterface::QX11Application>();
        //     if (xcbApp) {
        //         return xcbApp->display();
        //     }

        auto *ni = QGuiApplication::platformNativeInterface();
        return static_cast<xcb_connection_t *>(
            ni->nativeResourceForWindow("connection", windowhandle));
        break;
    }
    case LinuxDisplayType::WAYLAND: {
        auto *waylandApp = qGuiApp->nativeInterface<QNativeInterface::QWaylandApplication>();
        if (waylandApp) {
            return waylandApp->display();
        }
        break;
    }
    case LinuxDisplayType::UNKNOWN:
        break;
    }
    qWarning() << "could not get linux native display";

// idk wayland
#elif defined(Q_OS_WIN)
    HINSTANCE hinst = GetModuleHandle(NULL);
    return hinst;

#endif
    return nullptr;
}