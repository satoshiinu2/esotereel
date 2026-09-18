#pragma once
#include "esotereel_gui_helper.h"
#include <cstddef>
#include <stdexcept>
#include <string>

namespace esotereel {

template <typename T> using FfiOption = esotereel_gui_helper::FfiOption<T>;

template <typename T> class Option {
  public:
    Option() = default;

    explicit Option(FfiOption<T> raw) : raw_(raw) {}

    bool is_some() const noexcept {
        return raw_.has_value;
    }

    bool is_none() const noexcept {
        return !raw_.has_value;
    }

    const T &unwrap() const {
        if (!is_some())
            throw std::logic_error("FfiOption: unwrap() called on None");

        return raw_.value.some;
    }

    T &unwrap() {
        if (!is_some())
            throw std::logic_error("FfiOption: unwrap() called on None");

        return raw_.value.some;
    }

    const T &expect(const char *message) const {
        if (!is_some())
            throw std::logic_error(message);

        return raw_.value.some;
    }

    T unwrap_or(T fallback) const {
        if (is_some())
            return raw_.value.some;

        return fallback;
    }

    T unwrap_or_default() const
        requires std::default_initializable<T>
    {
        if (is_some())
            return raw_.value.some;

        return T{};
    }

    explicit operator bool() const noexcept {
        return is_some();
    }

    const FfiOption<T> &raw() const noexcept {
        return raw_;
    }

  private:
    FfiOption<T> raw_{};
};

} // namespace esotereel