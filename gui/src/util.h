#pragma once

#include <QtGlobal>
#include <algorithm>
#include <limits>
#include <qguiapplication_platform.h>
#include <qlogging.h>
#include <qwindow.h>
#include <set>
#include <vector>

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

LinuxDisplayType getLinuxDisplayType();
void *getNativeDisplay(QWindow *windowhandle);