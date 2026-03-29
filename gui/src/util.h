#pragma once

#include <QtGlobal>
#include <algorithm>
#include <limits>
#include <qguiapplication_platform.h>
#include <qlogging.h>
#include <qwindow.h>
#include <set>
#include <vector>

#if defined(Q_OS_WIN)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#elif defined(Q_OS_LINUX)
#include <qguiapplication_platform.h>
#include <qnativeinterface.h>
#include <qpa/qplatformnativeinterface.h>
#include <xcb/xcb.h>
#endif

template <typename T>
bool contains(const std::vector<T> &vec, const T &value) {
    return std::find(vec.begin(), vec.end(), value) != vec.end();
}
template <typename T>
bool contains(const std::set<T> &set, const T &value) {
    return set.find(value) != set.end();
}

template <typename T>
T sat_sub(T a, T b) {
    T res;
    // __builtin_sub_overflowはオーバーフロー検知用
    if (__builtin_sub_overflow(a, b, &res)) {
        // オーバーフロー（アンダーフロー）したら最小値を返す
        return std::numeric_limits<T>::min();
    }
    return res;
}

enum class LinuxDisplayType {
    UNKNOWN,
    XCB,
    WAYLAND
};

inline LinuxDisplayType getLinuxDisplayType() {
    QString platform = QGuiApplication::platformName();

    if (platform == "wayland") {
        return LinuxDisplayType::WAYLAND;
    }
    if (platform == "xcb") {
        return LinuxDisplayType::XCB;
    }
    return LinuxDisplayType::UNKNOWN;
};

inline void *
getNativeDisplay(QWindow *windowhandle) {
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