#pragma once
#include "esotereel_gui_helper.h"
#include "ffi/Array.h"
#include "ffi/Option.h"
#include "ffi/StringView.h"
#include <cstddef>
#include <optional>
#include <stdexcept>
#include <string>
#include <variant>

namespace {
class ResultErrorHolder {
  public:
    explicit ResultErrorHolder(std::string msg = {}) : msg(std::move(msg)) {}

    operator std::string() const {
        return msg;
    }

  private:
    std::string msg;
};

} // namespace

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
            value_ = ResultErrorHolder{OwnedString::intoStdString(raw.value.err)};
            return;
        }

        FfiArray<Raw> array = raw.value.ok;
        ArrayFreeGuard<Raw> guard{array};

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
            value_ = ResultErrorHolder{OwnedString::intoStdString(raw.value.err)};
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
            value_ = ResultErrorHolder{OwnedString::intoStdString(raw.value.err)};
            return;
        }

        value_ = converter(raw.value.ok);
    }

    explicit Result(FfiResult<T> raw) {
        if (raw.is_ok) {
            value_ = raw.value.ok;
        } else {
            value_ = ResultErrorHolder{OwnedString::intoStdString(raw.value.err)};
        }
    }

    static Result ok(T value) {
        return Result(std::move(value));
    }

    static Result err(std::string error) {
        return Result(ResultErrorHolder{std::move(error)});
    }

    bool isOk() const noexcept {
        return std::holds_alternative<T>(value_);
    }

    bool isError() const noexcept {
        return std::holds_alternative<ResultErrorHolder>(value_);
    }

    /// None の場合は例外
    T &unwrap() {
        if (isError()) {
            auto err = getError();
            assert(err.has_value());
            throw std::runtime_error(*err);
        }
        return std::get<T>(value_);
    }

    /// None の場合は例外
    const T &unwrap() const {
        if (isError()) {
            auto err = getError();
            assert(err.has_value());
            throw std::runtime_error(*err);
        }
        return std::get<T>(value_);
    }

    // ref-qualifierとかあるんや
    /// None の場合は fallback
    T unwrapOr(T fallback) const & {
        if (isOk()) {
            return std::get<T>(value_);
        }
        return fallback;
    }

    /// None の場合は fallback
    T unwrapOr(T fallback) && {
        if (isOk()) {
            return std::move(std::get<T>(value_));
        }
        return fallback;
    }

    // Compatibility method
    T unwrapOrMove() {
        return std::move(unwrap());
    }

    std::optional<std::string> getError() const {
        if (isOk()) {
            return std::nullopt;
        }
        return std::get<ResultErrorHolder>(value_);
    }

    template <typename F> auto map(F f) const -> Result<decltype(f(std::declval<const T &>()))> {
        using U = decltype(f(std::declval<const T &>()));
        if (isOk()) {
            return Result<U>::ok(f(std::get<T>(value_)));
        }
        auto err = getError();
        assert(err.has_value());
        return Result<U>::err(*err);
    }

    // Flatten Result<std::optional<T>> to std::optional<T>
    template <typename U = T, typename std::enable_if_t<std::is_same_v<U, std::optional<typename U::value_type>>, int> = 0>
    U flatten() const {
        if (isOk()) {
            return std::get<T>(value_);
        }
        return std::nullopt;
    }

  private:
    explicit Result(T value) : value_(std::move(value)) {}

    explicit Result(std::string error) : value_(ResultErrorHolder{std::move(error)}) {}

    std::variant<T, ResultErrorHolder> value_;
};

// Template specialization for Result<void>
template <> class Result<void> {
  public:
    explicit Result(FfiResultVoid raw) {
        if (raw.is_ok) {
            error_ = std::nullopt;
        } else {
            error_ = OwnedString::intoStdString(raw.err);
        }
    }

    static Result ok() {
        return Result();
    }

    static Result err(std::string error) {
        return Result(std::move(error));
    }

    bool isOk() const noexcept {
        return !error_.has_value();
    }

    bool isError() const noexcept {
        return error_.has_value();
    }

    // No-op unwrap for void compatibility
    void unwrap() const {
        if (isError()) {
            throw std::runtime_error(error_.value());
        }
    }

    std::optional<std::string> getError() const {
        return error_;
    }

  private:
    Result() : error_(std::nullopt) {}

    explicit Result(std::string error) : error_(std::move(error)) {}

    std::optional<std::string> error_;
};
} // namespace esotereel