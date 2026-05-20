#include "exception.h"
#include "esotereel_gui_helper.h"
#include "stringview.h"

using WrapperResult = esotereel_gui_helper::WrapperResult;
using WrapperErrorCode = esotereel_gui_helper::WrapperErrorCode;

bool checkWrapperResult(WrapperResult result) {
    switch (result.code) {
    case WrapperErrorCode::Ok:
    case WrapperErrorCode::NotFound:
        break;

    case WrapperErrorCode::Error:
        throw WrapperException(StringView::toStdString(result.message), result.code);

    case WrapperErrorCode::NullPtr:
    case WrapperErrorCode::Panic:
        throw WrapperFatalException(StringView::toStdString(result.message), result.code);
    }

    return result.code == WrapperErrorCode::Ok;
}