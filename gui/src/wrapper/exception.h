#pragma once
#include <stdexcept>
#include <string>
#include "result.h"
#include "esotereel_gui_helper.h"

namespace esotereel_gui_helper {
enum class WrapperErrorCode;
struct WrapperResult;
} // namespace esotereel_gui_helper

using WrapperErrorCode = esotereel_gui_helper::WrapperErrorCode;

// return true if result is ok
bool checkWrapperResult(WrapperErrorCode result);

// Convert WrapperErrorCode to Result<T>
template<typename T>
esotereel_gui_helper::Result<T> wrapperResultToResult(WrapperErrorCode code, T value = T{}) {
    const char *msg = esotereel_gui_helper::get_last_err_msg();
    
    switch (code) {
    case WrapperErrorCode::Ok:
        return esotereel_gui_helper::Result<T>::ok(std::move(value));
    case WrapperErrorCode::NotFound:
        return esotereel_gui_helper::Result<T>::error(std::string("Not found: ") + (msg ? msg : ""));
    case WrapperErrorCode::Error:
        return esotereel_gui_helper::Result<T>::error(std::string("Error: ") + (msg ? msg : ""));
    case WrapperErrorCode::NullPtr:
        return esotereel_gui_helper::Result<T>::error(std::string("Null pointer: ") + (msg ? msg : ""));
    case WrapperErrorCode::Panic:
        return esotereel_gui_helper::Result<T>::error(std::string("Panic: ") + (msg ? msg : ""));
    default:
        return esotereel_gui_helper::Result<T>::error(std::string("Unknown error"));
    }
}

// Specialization for void
inline esotereel_gui_helper::Result<void> wrapperResultToResultVoid(WrapperErrorCode code) {
    const char *msg = esotereel_gui_helper::get_last_err_msg();
    
    switch (code) {
    case WrapperErrorCode::Ok:
        return esotereel_gui_helper::Result<void>::ok();
    case WrapperErrorCode::NotFound:
        return esotereel_gui_helper::Result<void>::error(std::string("Not found: ") + (msg ? msg : ""));
    case WrapperErrorCode::Error:
        return esotereel_gui_helper::Result<void>::error(std::string("Error: ") + (msg ? msg : ""));
    case WrapperErrorCode::NullPtr:
        return esotereel_gui_helper::Result<void>::error(std::string("Null pointer: ") + (msg ? msg : ""));
    case WrapperErrorCode::Panic:
        return esotereel_gui_helper::Result<void>::error(std::string("Panic: ") + (msg ? msg : ""));
    default:
        return esotereel_gui_helper::Result<void>::error(std::string("Unknown error"));
    }
}

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
