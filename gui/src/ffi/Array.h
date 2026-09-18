#pragma once
#include "esotereel_gui_helper.h"
#include <cstddef>

namespace esotereel {

template <typename T> using FfiArray = esotereel_gui_helper::FfiArray<T>;

template <typename T> class Array {
  private:
    FfiArray<T> raw_;

  public:
    explicit Array(FfiArray<T> raw) : raw_(raw) {}

    Array(const Array &) = delete;
    Array &operator=(const Array &) = delete;

    Array(Array &&other) noexcept : raw_(other.raw_) {
        other.raw_ = {};
    }

    Array &operator=(Array &&other) noexcept {
        if (this != &other) {
            reset();

            raw_ = other.raw_;
            other.raw_ = {};
        }
        return *this;
    }

    ~Array() {
        reset();
    }

    size_t size() const noexcept {
        return raw_.len;
    }

    bool empty() const noexcept {
        return raw_.len == 0;
    }

    T *data() noexcept {
        return raw_.ptr;
    }

    const T *data() const noexcept {
        return raw_.ptr;
    }

    T &operator[](size_t index) noexcept {
        return raw_.ptr[index];
    }

    const T &operator[](size_t index) const noexcept {
        return raw_.ptr[index];
    }

  private:
    void reset() noexcept {
        if (raw_.free_fn) {
            raw_.free_fn(&raw_);
        }

        raw_ = {};
    }
};
} // namespace esotereel