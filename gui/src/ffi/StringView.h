#pragma once

#include "esotereel_gui_helper.h"
#include <QString>
#include <cstdint>
#include <qobject.h>
#include <string>

using RawStringView = esotereel_gui_helper::StringView;
using RawOwnedString = esotereel_gui_helper::OwnedString;

namespace esotereel::StringView {
inline bool isZero(const RawStringView &raw) {
    return raw.ptr == nullptr || raw.len == 0;
}

inline RawStringView zero() {
    return {nullptr, static_cast<size_t>(0)};
}

inline std::string toStdString(const RawStringView &raw) {
    return std::string(reinterpret_cast<const char *>(raw.ptr), raw.len);
}

inline QString toQString(const RawStringView &raw) {
    if (!raw.ptr || raw.len == 0) {
        return QString();
    }

    return QString::fromUtf8(reinterpret_cast<const char *>(raw.ptr), static_cast<int>(raw.len));
}

inline RawStringView fromStdString(const std::string &str) {
    if (str.empty()) {
        return {nullptr, 0};
    }
    return {reinterpret_cast<const uint8_t *>(str.data()), str.size()};
}

inline RawStringView fromQUtf8String(const QByteArray &utf8) {
    return {reinterpret_cast<const uint8_t *>(utf8.constData()), static_cast<size_t>(utf8.size())};
}

}; // namespace esotereel::StringView

namespace esotereel::OwnedString {

inline void free(const RawOwnedString &raw) {
    esotereel_gui_helper::owned_string_free(raw);
}

inline bool isNull(const RawOwnedString &raw) {
    return raw.ptr == nullptr || raw.len == 0;
}

inline RawOwnedString zero() {
    return {nullptr, static_cast<size_t>(0)};
}

inline std::string toStdString(const RawOwnedString &raw) {
    return std::string(reinterpret_cast<const char *>(raw.ptr), raw.len);
}

inline std::string intoStdString(const RawOwnedString &raw) {
    auto str = toStdString(raw);
    free(raw);
    return str;
}

inline QString toQString(const RawOwnedString &raw) {
    if (!raw.ptr || raw.len == 0) {
        return QString();
    }

    return QString::fromUtf8(reinterpret_cast<const char *>(raw.ptr), static_cast<int>(raw.len));
}

inline QString intoQString(const RawOwnedString &raw) {
    auto str = toQString(raw);
    free(raw);
    return str;
}

inline RawOwnedString fromStdString(const std::string &str) {
    if (str.empty()) {
        return {nullptr, 0};
    }
    return esotereel_gui_helper::owned_string_new(StringView::fromStdString(str));
}

inline RawOwnedString fromQUtf8String(const QByteArray &utf8) {
    return esotereel_gui_helper::owned_string_new(StringView::fromQUtf8String(utf8));
}

}; // namespace esotereel::OwnedString