#pragma once

#include "esotereel_gui_helper.h"
#include <QString>
#include <cstdint>
#include <qobject.h>
#include <string>

using RawStringView = esotereel_gui_helper::FfiStringView;
using RawOwnedString = esotereel_gui_helper::FfiOwnedString;

namespace esotereel::StringView {

inline bool isZero(const RawStringView &raw) noexcept {
    return raw.ptr == nullptr || raw.len == 0;
}

inline RawStringView zero() noexcept {
    return {nullptr, static_cast<size_t>(0)};
}

inline std::string toStdString(const RawStringView &raw) noexcept {
    if (!raw.ptr || raw.len == 0) {
        return std::string();
    }
    // Defensive check: prevent excessively long strings that could indicate corruption
    if (raw.len > 100 * 1024 * 1024) { // 100 MB
        return std::string();
    }
    return std::string(reinterpret_cast<const char *>(raw.ptr), raw.len);
}

inline QString toQString(const RawStringView &raw) noexcept {
    if (!raw.ptr || raw.len == 0) {
        return QString();
    }
    // Defensive check: prevent excessively long strings that could indicate corruption
    if (raw.len > 100 * 1024 * 1024) { // 100 MB
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

} // namespace esotereel::StringView

namespace esotereel::OwnedString {

inline void free(const RawOwnedString &raw) {
    esotereel_gui_helper::owned_string_free(raw);
}

inline bool isNull(const RawOwnedString &raw) noexcept {
    return raw.ptr == nullptr || raw.len == 0;
}

inline RawOwnedString zero() noexcept {
    return {nullptr, static_cast<size_t>(0)};
}

inline std::string toStdString(const RawOwnedString &raw) noexcept {
    if (!raw.ptr || raw.len == 0) {
        return std::string();
    }
    // Defensive check: prevent excessively long strings that could indicate corruption
    if (raw.len > 100 * 1024 * 1024) { // 100 MB
        return std::string();
    }
    return std::string(reinterpret_cast<const char *>(raw.ptr), raw.len);
}

inline std::string intoStdString(RawOwnedString raw) {
    std::string str = toStdString(raw);
    free(raw);
    return str;
}

inline QString toQString(const RawOwnedString &raw) noexcept {
    if (!raw.ptr || raw.len == 0) {
        return QString();
    }
    // Defensive check: prevent excessively long strings that could indicate corruption
    if (raw.len > 100 * 1024 * 1024) { // 100 MB
        return QString();
    }
    return QString::fromUtf8(reinterpret_cast<const char *>(raw.ptr), static_cast<int>(raw.len));
}

inline QString intoQString(RawOwnedString raw) {
    QString str = toQString(raw);
    free(raw);
    return str;
}

inline RawOwnedString fromStdString(const std::string &str) {
    if (str.empty()) {
        return esotereel_gui_helper::owned_string_new(StringView::fromStdString(str));
    }
    return esotereel_gui_helper::owned_string_new(StringView::fromStdString(str));
}

inline RawOwnedString fromQUtf8String(const QByteArray &utf8) {
    if (utf8.isEmpty()) {
        return esotereel_gui_helper::owned_string_new(StringView::zero());
    }
    return esotereel_gui_helper::owned_string_new(StringView::fromQUtf8String(utf8));
}

} // namespace esotereel::OwnedString