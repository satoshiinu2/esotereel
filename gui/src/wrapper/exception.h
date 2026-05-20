#pragma once
#include <stdexcept>
#include <string>

namespace esotereel_gui_helper {
enum class WrapperErrorCode;
struct WrapperResult;
} // namespace esotereel_gui_helper

using WrapperResult = esotereel_gui_helper::WrapperResult;
using WrapperErrorCode = esotereel_gui_helper::WrapperErrorCode;

// return true if result is ok
bool checkWrapperResult(WrapperResult result);

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
