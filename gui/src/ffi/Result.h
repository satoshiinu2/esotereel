#pragma once
#include "esotereel_gui_helper.h"
#include "ffi/Array.h"
#include "ffi/Option.h"
#include "ffi/StringView.h"
#include <cstddef>
#include <stdexcept>
#include <string>
#include <variant>

namespace esotereel {
template <typename T> using FfiArray = esotereel_gui_helper::FfiArray<T>;
template <typename T> using FfiOption = esotereel_gui_helper::FfiOption<T>;
template <typename T> using FfiResult = esotereel_gui_helper::FfiResult<T>;
using FfiResultVoid = esotereel_gui_helper::FfiResultVoid;

template <typename T> class Result {
  public:
    template <typename Raw, typename Converter> Result(FfiResult<FfiArray<Raw>> raw, Converter converter) {
        using Converted = decltype(converter(std::declval<const Raw &>()));

        if (!raw.is_ok) {
            value_ = OwnedString::intoStdString(raw.value.err);
            return;
        }

        FfiArray<Raw> array = raw.value.ok;
        detail::ArrayFreeGuard<Raw> guard{array};

        QVector<Converted> out;
        const size_t n = Array::size(array);
        out.reserve(static_cast<int>(n));
        const Raw *data = Array::data(array);
        for (size_t i = 0; i < n; ++i) {
            out.append(converter(data[i]));
        }

        value_ = std::move(out);
    }

    template <typename Raw, typename Converter> Result(FfiResult<FfiOption<Raw>> raw, Converter converter) {
        using Converted = decltype(converter(std::declval<const Raw &>()));

        if (!raw.is_ok) {
            value_ = OwnedString::intoStdString(raw.value.err);
            return;
        }

        if (Option::isNone(raw.value.ok)) {
            value_ = std::nullopt;
            return;
        }

        value_ = converter(Option::unwrap(raw.value.ok));
    }

    template <typename Raw, typename Converter> Result(FfiResult<Raw> raw, Converter converter) {
        using Converted = decltype(converter(std::declval<const Raw &>()));

        if (!raw.is_ok) {
            value_ = OwnedString::intoStdString(raw.value.err);
            return;
        }

        value_ = converter(raw.value.ok);
    }

    explicit Result(FfiResult<T> raw) {
        if (raw.is_ok) {
            value_ = raw.value.ok;
        } else {
            value_ = OwnedString::intoStdString(raw.value.err);
        }
    }

    static Result ok(T value) {
        return Result(std::move(value));
    }

    static Result err(std::string error) {
        return Result(std::move(error));
    }

    bool isOk() const noexcept {
        return std::holds_alternative<T>(value_);
    }

    bool isError() const noexcept {
        return std::holds_alternative<std::string>(value_);
    }

    T &unwrap() {
        return std::get<T>(value_);
    }

    const T &unwrap() const {
        return std::get<T>(value_);
    }

    // Compatibility method
    T unwrapOrMove() {
        return std::move(unwrap());
    }

    const std::string &error() const {
        return std::get<std::string>(value_);
    }

  private:
    explicit Result(T value) : value_(std::move(value)) {}

    explicit Result(std::string error) : value_(std::move(error)) {}

    std::variant<T, std::string> value_;
};

class ResultVoid {
  public:
    explicit ResultVoid(FfiResultVoid raw) {
        if (raw.is_ok) {
            ok_ = true;
        } else {
            error_ = OwnedString::intoStdString(raw.err);
            ok_ = false;
        }
    }

    static ResultVoid ok() {
        return ResultVoid();
    }

    static ResultVoid err(std::string error) {
        return ResultVoid(std::move(error));
    }

    bool is_ok() const noexcept {
        return ok_;
    }

    bool is_err() const noexcept {
        return !ok_;
    }

    // Compatibility aliases
    bool isError() const noexcept {
        return is_err();
    }

    const std::string &error() const {
        return error_;
    }

  private:
    ResultVoid() : ok_(true) {}

    explicit ResultVoid(std::string error) : ok_(false), error_(std::move(error)) {}

    bool ok_;
    std::string error_;
};

// Template specialization for Result<void>
template <> class Result<void> {
  public:
    explicit Result(FfiResultVoid raw) {
        if (raw.is_ok) {
            ok_ = true;
        } else {
            error_ = OwnedString::intoStdString(raw.err);
            ok_ = false;
        }
    }

    static Result ok() {
        return Result();
    }

    static Result err(std::string error) {
        return Result(std::move(error));
    }

    bool is_ok() const noexcept {
        return ok_;
    }

    bool is_err() const noexcept {
        return !ok_;
    }

    // Compatibility aliases
    bool isError() const noexcept {
        return is_err();
    }

    // No-op unwrap for void compatibility
    void unwrap() const {
        if (is_err()) {
            throw std::runtime_error(error_);
        }
    }

    const std::string &error() const {
        return error_;
    }

  private:
    Result() : ok_(true) {}

    explicit Result(std::string error) : ok_(false), error_(std::move(error)) {}

    bool ok_;
    std::string error_;
};
} // namespace esotereel