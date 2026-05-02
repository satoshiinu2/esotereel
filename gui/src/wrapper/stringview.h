#pragma once

#include "esotereel_gui_helper.h"
#include <QString>
#include <cstdint>
#include <string>

using RawStringView = esotereel_gui_helper::_StringView;

namespace StringView {
static std::string toStdString(const RawStringView &raw) {
    return std::string(reinterpret_cast<const char *>(raw.ptr), raw.len);
}

static QString toQstring(const RawStringView &raw) {
    if (!raw.ptr || raw.len == 0) {
        return QString();
    }

    return QString::fromUtf8(reinterpret_cast<const char *>(raw.ptr), static_cast<int>(raw.len));
}

static RawStringView fromStdString(const std::string &str) {
    if(str.empty()) {
        return {nullptr, 0};
    }
    return {reinterpret_cast<const uint8_t *>(str.data()), str.size()};
}

static RawStringView fromQstring(const QString &str) {
    if (str.isEmpty()) {
        return {nullptr, 0};
    }
    QByteArray utf8 = str.toUtf8();
    return {reinterpret_cast<const uint8_t *>(utf8.constData()), static_cast<size_t>(utf8.size())};
}

}; // namespace StringView