#pragma once
#include "esotereel_gui_helper.h"

namespace esotereel {
template <typename T> using FfiOption = esotereel_gui_helper::FfiOption<T>;
}

namespace esotereel::Option {

template <typename T> inline bool isSome(const FfiOption<T> &opt) noexcept {
    return opt.has_value;
}

template <typename T> inline bool isNone(const FfiOption<T> &opt) noexcept {
    return !opt.has_value;
}

template <typename T> inline const T &unwrap(const FfiOption<T> &opt) noexcept {
    return opt.value.some;
}

template <typename T> inline T unwrapOr(const FfiOption<T> &opt, T fallback) noexcept {
    return opt.has_value ? opt.value.some : fallback;
}

} // namespace esotereel::Option
