#pragma once
#include "esotereel_gui_helper.h"
#include <optional>
#include <stdexcept>
#include <string>
#include <variant>

namespace esotereel {

template <typename T> class Result {
  private:
    std::variant<T, std::string> value;

  public:
    // Constructors
    Result(T val) : value(std::move(val)) {}
    Result(std::string err) : value(std::move(err)) {}

    // Static factory methods
    static Result ok(T val) {
        return Result(std::move(val));
    }

    static Result error(std::string msg) {
        return Result(std::move(msg));
    }

    // Check if result is ok
    bool isOk() const {
        return std::holds_alternative<T>(value);
    }

    bool isError() const {
        return std::holds_alternative<std::string>(value);
    }

    // Get the value (returns optional)
    std::optional<T> okValue() const {
        if (isOk()) {
            return std::get<T>(value);
        }
        return std::nullopt;
    }

    // Get the error message (returns optional)
    std::optional<std::string> errorMessage() const {
        if (isError()) {
            return std::get<std::string>(value);
        }
        return std::nullopt;
    }

    // Unwrap - throws if error
    T &unwrap() {
        if (isError()) {
            throw std::runtime_error(std::get<std::string>(value));
        }
        return std::get<T>(value);
    }

    const T &unwrap() const {
        if (isError()) {
            throw std::runtime_error(std::get<std::string>(value));
        }
        return std::get<T>(value);
    }

    // Unwrap or move
    T unwrapOrMove() {
        if (isError()) {
            throw std::runtime_error(std::get<std::string>(value));
        }
        return std::get<T>(std::move(value));
    }

    // Unwrap or default
    T unwrapOr(T default_val) const {
        if (isOk()) {
            return std::get<T>(value);
        }
        return default_val;
    }

    // Map function for transforming success values
    template <typename F> auto map(F &&f) const -> Result<decltype(f(std::declval<T>()))> {
        if (isOk()) {
            return Result::ok(f(std::get<T>(value)));
        }
        return Result::error(std::get<std::string>(value));
    }

    // And then for chaining
    template <typename F> auto andThen(F &&f) const -> decltype(f(std::declval<T>())) {
        if (isOk()) {
            return f(std::get<T>(value));
        }
        return decltype(f(std::declval<T>()))::error(std::get<std::string>(value));
    }
};

// Specialization for void
template <> class Result<void> {
  private:
    std::optional<std::string> error_msg;

  public:
    Result() : error_msg(std::nullopt) {}
    Result(std::string err) : error_msg(std::move(err)) {}

    static Result ok() {
        return Result();
    }

    static Result error(std::string msg) {
        return Result(std::move(msg));
    }

    bool isOk() const {
        return !error_msg.has_value();
    }

    bool isError() const {
        return error_msg.has_value();
    }

    std::optional<std::string> errorMessage() const {
        return error_msg;
    }

    void unwrap() const {
        if (isError()) {
            throw std::runtime_error(*error_msg);
        }
    }
};

} // namespace esotereel