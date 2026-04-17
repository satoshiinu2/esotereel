#pragma once

#include "esotereel_gui_helper.h"
#include <QString>
#include <cstdint>
#include <string>

using RawStringView = esotereel_gui_helper::_StringView;

struct StringView {
    const uint8_t *ptr;
    size_t len;

    StringView(const RawStringView &raw) : ptr(raw.ptr), len(raw.len) {
    }

    std::string
    toStdString() const {
        return std::string(reinterpret_cast<const char *>(ptr), len);
    }

    QString toQstring() const {
        if (!ptr || len == 0) {
            return QString();
        }

        return QString::fromUtf8(reinterpret_cast<const char *>(ptr), static_cast<int>(len));
    }

    static QString toQstring(const RawStringView &raw) {
        if (!raw.ptr || raw.len == 0) {
            return QString();
        }

        return QString::fromUtf8(reinterpret_cast<const char *>(raw.ptr), static_cast<int>(raw.len));
    }

    bool isValid() const noexcept { return ptr != nullptr; }
};