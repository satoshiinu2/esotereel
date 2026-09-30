#pragma once

#include "esotereel_gui_helper.h"
#include <cstddef>
#include <type_traits>
#include <utility>
#include <vector>

namespace esotereel {
template <typename T> using FfiArray = esotereel_gui_helper::FfiArray<T>;
}

namespace esotereel::Array {

template <typename T, void (*FreeElement)(const T &) = nullptr> inline void freeCppArray(FfiArray<T> *raw) noexcept {
    if (raw == nullptr) {
        return;
    }

    if (raw->ptr != nullptr) {
        if constexpr (FreeElement != nullptr) {
            for (size_t i = 0; i < raw->len; ++i) {
                FreeElement(raw->ptr[i]);
            }
        }
        delete[] raw->ptr;
    }

    raw->ptr = nullptr;
    raw->len = 0;
    raw->cap = 0;
}

// std::vectorの内容をC++所有の配列へ移し、配列用free_fn付きFfiArrayを作る。
// FreeElementを指定すると、各要素を解放してから配列本体をdelete[]する。
template <typename T, void (*FreeElement)(const T &) = nullptr> inline FfiArray<T> fromVector(std::vector<T> &&values) {
    static_assert(std::is_default_constructible_v<T>);
    static_assert(std::is_nothrow_move_assignable_v<T>, "FfiArray::fromVector requires nothrow move assignment");

    const size_t len = values.size();
    T *ptr = nullptr;
    try {
        ptr = len == 0 ? nullptr : new T[len];
    } catch (...) {
        if constexpr (FreeElement != nullptr) {
            for (const auto &value : values) {
                FreeElement(value);
            }
        }
        throw;
    }

    for (size_t i = 0; i < len; ++i) {
        ptr[i] = std::move(values[i]);
        values[i] = T{};
    }

    return FfiArray<T>{ptr, len, len, freeCppArray<T, FreeElement>};
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

// 配列の生成元に対応するfree_fnで解放する。使い終わったら必ずfree()を
// 呼ぶこと（呼び忘れるとリーク）。free_fnは例外を外へ送出してはならない。
template <typename T> inline void free(FfiArray<T> &raw) {
    if (raw.free_fn) {
        raw.free_fn(&raw);
    }
    raw.ptr = nullptr;
    raw.len = 0;
    raw.cap = 0;
}

} // namespace esotereel::Array

namespace esotereel {
// converterが例外を投げてもFfiArrayを解放するための最小限のスコープガード。
// 公開APIではなくconvertArrayResult内部だけの実装詳細。
template <typename T> struct ArrayFreeGuard {
    FfiArray<T> &raw;

    explicit ArrayFreeGuard(FfiArray<T> &raw) : raw(raw) {}

    ~ArrayFreeGuard() {
        Array::free(raw);
    }

    const T *data() const noexcept {
        return Array::data(raw);
    }
};
} // namespace esotereel