#pragma once
#include "esotereel_gui_helper.h"
#include "ffi/StringView.h"
#include <cstddef>
#include <stdexcept>
#include <string>
#include <variant>

namespace esotereel {

template <typename T> using FfiResult = esotereel_gui_helper::FfiResult<T>;
using FfiResultVoid = esotereel_gui_helper::FfiResultVoid;

template <typename T> class Result {
  public:
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

    bool is_ok() const noexcept {
        return std::holds_alternative<T>(value_);
    }

    bool is_err() const noexcept {
        return std::holds_alternative<std::string>(value_);
    }

    // Compatibility aliases
    bool isError() const noexcept {
        return is_err();
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
template <>
class Result<void> {
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