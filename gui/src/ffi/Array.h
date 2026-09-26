#pragma once

#include "esotereel_gui_helper.h"
#include <cstddef>

namespace esotereel {
template <typename T> using FfiArray = esotereel_gui_helper::FfiArray<T>;
}

namespace esotereel::Array {

template <typename T> inline FfiArray<T> from(std::span<T> span) noexcept {
    return {span.data(), span.size(), span.size()};
}

template <typename T> inline size_t size(const FfiArray<T> &raw) noexcept {
    return raw.len;
}

template <typename T> inline bool isEmpty(const FfiArray<T> &raw) noexcept {
    return raw.len == 0;
}

template <typename T> inline const T *data(const FfiArray<T> &raw) noexcept {
    return raw.ptr;
}

template <typename T> inline T *data(FfiArray<T> &raw) noexcept {
    return raw.ptr;
}

// OwnedString::free と同じノリ: FfiArrayはRustのVecを借りているだけなので、
// 使い終わったら必ずfree()を呼ぶこと（呼び忘れるとリーク）。
template <typename T> inline void free(FfiArray<T> &raw) {
    if (raw.free_fn) {
        raw.free_fn(&raw);
    }
    raw.ptr = nullptr;
    raw.len = 0;
    raw.cap = 0;
}

} // namespace esotereel::Array

namespace esotereel::detail {
// converterが例外を投げてもFfiArrayを解放するための最小限のスコープガード。
// 公開APIではなくconvertArrayResult内部だけの実装詳細。
template <typename T> struct ArrayFreeGuard {
    FfiArray<T> &raw;
    ~ArrayFreeGuard() {
        Array::free(raw);
    }
};
} // namespace esotereel::detail