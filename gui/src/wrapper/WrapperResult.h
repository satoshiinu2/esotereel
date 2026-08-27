#pragma once
#include "esotereel_gui_helper.h"
#include "wrapper/Result.h"
#include <optional>
#include <stdexcept>
#include <string>
#include <variant>

namespace esotereel_gui_helper {
enum class WrapperErrorCode;
struct WrapperResult;
} // namespace esotereel_gui_helper

namespace esotereel {
using WrapperErrorCode = esotereel_gui_helper::WrapperErrorCode;

class WrapperException : public std::runtime_error {
  public:
    WrapperException(std::string msg, WrapperErrorCode errCode) : errCode(errCode), runtime_error(msg) {}

    // enum type
    WrapperErrorCode errCode;
};

class WrapperFatalException : public std::runtime_error {
  public:
    WrapperFatalException(std::string msg, WrapperErrorCode errCode) : errCode(errCode), runtime_error(msg) {}

    // enum type
    WrapperErrorCode errCode;
};

// return true if result is ok
inline bool checkWrapperResult(WrapperErrorCode code) {

    const char *msg = esotereel_gui_helper::get_last_err_msg();
    switch (code) {
    case WrapperErrorCode::Ok:
    case WrapperErrorCode::NotFound:
        break;

    case WrapperErrorCode::Error:
        qCritical() << "Wrapper error [Error]: " << msg;
        throw WrapperException(msg, code);

    case WrapperErrorCode::NullPtr:
        qCritical() << "Wrapper error [NullPtr]: " << msg;
        throw WrapperFatalException(msg, code);
    case WrapperErrorCode::Panic:
        qCritical() << "Wrapper error [Panic]: " << msg;
        throw WrapperFatalException(msg, code);
    }

    return code == WrapperErrorCode::Ok;
}

// Convert WrapperErrorCode to Result<T>
template <typename T> Result<T> wrapperResultToResult(WrapperErrorCode code, T value = T{}) {
    const char *msg = esotereel_gui_helper::get_last_err_msg();

    switch (code) {
    case WrapperErrorCode::Ok:
        return Result<T>::ok(std::move(value));
    case WrapperErrorCode::NotFound:
        return Result<T>::error(std::string("Not found: ") + (msg ? msg : ""));
    case WrapperErrorCode::Error:
        return Result<T>::error(std::string("Error: ") + (msg ? msg : ""));
    case WrapperErrorCode::NullPtr:
        return Result<T>::error(std::string("Null pointer: ") + (msg ? msg : ""));
    case WrapperErrorCode::Panic:
        return Result<T>::error(std::string("Panic: ") + (msg ? msg : ""));
    default:
        return Result<T>::error(std::string("Unknown error"));
    }
}

// Specialization for void
inline Result<void> wrapperResultToResultVoid(WrapperErrorCode code) {
    const char *msg = esotereel_gui_helper::get_last_err_msg();

    switch (code) {
    case WrapperErrorCode::Ok:
        return Result<void>::ok();
    case WrapperErrorCode::NotFound:
        return Result<void>::error(std::string("Not found: ") + (msg ? msg : ""));
    case WrapperErrorCode::Error:
        return Result<void>::error(std::string("Error: ") + (msg ? msg : ""));
    case WrapperErrorCode::NullPtr:
        return Result<void>::error(std::string("Null pointer: ") + (msg ? msg : ""));
    case WrapperErrorCode::Panic:
        return Result<void>::error(std::string("Panic: ") + (msg ? msg : ""));
    default:
        return Result<void>::error(std::string("Unknown error"));
    }
}
} // namespace esotereel