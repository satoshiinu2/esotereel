#pragma once

#include "esotereel_gui_helper.h"
#include "stringview.h"
#include <QString>

using WrapperErrorCode = esotereel_gui_helper::_WrapperErrorCode;

class InternalServer {
  public:
    static bool internalServerStart(QString addr) {
        QByteArray addrUtf8 = addr.toUtf8();
        auto addrView = StringView::fromQUtf8String(addrUtf8);

        auto res = esotereel_gui_helper::internal_server_start(addrView);
        return res == WrapperErrorCode::Ok;
    }
};