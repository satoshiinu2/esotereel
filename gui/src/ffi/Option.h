#pragma once

#include "esotereel_gui_helper.h"

#include <optional>
#include <stdexcept>
#include <utility>

namespace esotereel {

template <typename T> using FfiOption = esotereel_gui_helper::FfiOption<T>;

} // namespace esotereel

namespace esotereel::Option {

template <typename T> inline bool isSome(const FfiOption<T> &opt) noexcept {
    return opt.has_value;
}

template <typename T> inline bool isNone(const FfiOption<T> &opt) noexcept {
    return !opt.has_value;
}

/// FFI Option -> std::optional
template <typename T> inline std::optional<T> asOptional(const FfiOption<T> &opt) {
    if (isNone(opt)) {
        return std::nullopt;
    }

    return opt.value.some;
}

/// None の場合は例外
template <typename T> inline const T &unwrap(const FfiOption<T> &opt) {
    if (isNone(opt)) {
        throw std::runtime_error("called unwrap on None");
    }

    return opt.value.some;
}

/// None の場合は fallback
template <typename T> inline T unwrapOr(const FfiOption<T> &opt, T fallback) {
    if (isNone(opt)) {
        return fallback;
    }

    return opt.value.some;
}

} // namespace esotereel::Option