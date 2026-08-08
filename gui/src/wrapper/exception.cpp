#include "exception.h"
#include "QDebug"
#include "esotereel_gui_helper.h"
#include "stringview.h"

using WrapperErrorCode = esotereel_gui_helper::WrapperErrorCode;

bool checkWrapperResult(WrapperErrorCode code) {

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