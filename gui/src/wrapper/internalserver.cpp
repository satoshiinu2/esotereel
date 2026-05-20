#include "internalserver.h"
#include "esotereel_gui_helper.h"
#include "stringview.h"

using WrapperResult = esotereel_gui_helper::WrapperResult;

bool InternalServer::start(QString addr, void (*OnConnectedFn)(bool)) {
    QByteArray addrUtf8 = addr.toUtf8();
    auto addrView = StringView::fromQUtf8String(addrUtf8);

    auto res = esotereel_gui_helper::internal_server_start(addrView, OnConnectedFn);

    return res == WrapperResult::Ok;
}