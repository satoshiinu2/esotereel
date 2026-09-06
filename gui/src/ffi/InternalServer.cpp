#include "InternalServer.h"
#include "Result.h"
#include "StringView.h"
#include "esotereel_gui_helper.h"
#include "ffi/WrapperResult.h"

namespace esotereel {
bool InternalServer::start(QString addr, void (*OnConnectedFn)(bool), QString stdPluginDir, QString workingDir) {
    QByteArray addrUtf8 = addr.toUtf8();
    auto addrView = StringView::fromQUtf8String(addrUtf8);

    QByteArray stdPluginDirUtf8 = stdPluginDir.toUtf8();
    auto stdPluginDirView = StringView::fromQUtf8String(stdPluginDirUtf8);

    QByteArray workingDirUtf8 = workingDir.toUtf8();
    auto workingDirView = StringView::fromQUtf8String(workingDirUtf8);

    auto result = esotereel_gui_helper::internal_server_start(addrView, OnConnectedFn, stdPluginDirView, workingDirView);

    return checkWrapperResult(result);
}
} // namespace esotereel