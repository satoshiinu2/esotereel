#include "InternalServer.h"
#include "Result.h"
#include "StringView.h"
#include "esotereel_gui_helper.h"
#include "wrapper/WrapperResult.h"

namespace esotereel {
bool InternalServer::start(QString addr, void (*OnConnectedFn)(bool)) {
    QByteArray addrUtf8 = addr.toUtf8();
    auto addrView = StringView::fromQUtf8String(addrUtf8);

    auto result = esotereel_gui_helper::internal_server_start(addrView, OnConnectedFn);

    return checkWrapperResult(result);
}
} // namespace esotereel