#include "exception.h"
#include "esotereel_gui_helper.h"
#include "stringview.h"
#include "QDebug"

using WrapperResult = esotereel_gui_helper::WrapperResult;
using WrapperErrorCode = esotereel_gui_helper::WrapperErrorCode;

bool checkWrapperResult(WrapperResult result) {
    switch (result.code) {
    case WrapperErrorCode::Ok:
    case WrapperErrorCode::NotFound:
        break;

    case WrapperErrorCode::Error:
        qCritical() << "Wrapper error Error: " << StringView::toStdString(result.message).c_str();
        throw WrapperException(StringView::toStdString(result.message), result.code);

    case WrapperErrorCode::NullPtr:
        qCritical() << "Wrapper error NullPtr: " << StringView::toStdString(result.message).c_str();
        throw WrapperFatalException(StringView::toStdString(result.message), result.code);
    case WrapperErrorCode::Panic:
        qCritical() << "Wrapper error Panic: " << StringView::toStdString(result.message).c_str();
        throw WrapperFatalException(StringView::toStdString(result.message), result.code);
    }

    return result.code == WrapperErrorCode::Ok;
}