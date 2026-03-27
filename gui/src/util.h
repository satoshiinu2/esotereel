#pragma once

#include <QtGlobal>
#include <algorithm>
#include <limits>
#include <qguiapplication_platform.h>
#include <qnativeinterface.h>
#include <set>
#include <vector>
#include <wayland-client.h>

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

#include <QGuiApplication>
#include <qnativeinterface.h>
#include <qpa/qplatformnativeinterface.h>
#include <wayland-client.h>
inline void *getNativeDisplay() {
#if defined(Q_OS_LINUX)
    auto *x11App = qGuiApp->nativeInterface<QNativeInterface::QX11Application>();
    if (x11App) {
        return x11App->display();
    }

    // idk wayland

#endif
    return nullptr;
}