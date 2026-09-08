#include "InternalServer.h"
#include "Result.h"
#include "StringView.h"
#include "esotereel_gui_helper.h"
#include "ffi/ClientNetworkHandler.h"
#include "ffi/WrapperResult.h"

namespace esotereel {
bool InternalServer::start(ClientNetworkHandler &network, QString addr,
                           void (*OnConnectedFn)(bool, esotereel_gui_helper::StringView), QString stdPluginDir,
                           QString workingDir) {
    QByteArray addrUtf8 = addr.toUtf8();
    auto addrView = StringView::fromQUtf8String(addrUtf8);

    QByteArray stdPluginDirUtf8 = stdPluginDir.toUtf8();
    auto stdPluginDirView = StringView::fromQUtf8String(stdPluginDirUtf8);

    QByteArray workingDirUtf8 = workingDir.toUtf8();
    auto workingDirView = StringView::fromQUtf8String(workingDirUtf8);

    auto result =
        esotereel_gui_helper::internal_server_start(network, addrView, OnConnectedFn, stdPluginDirView, workingDirView);

    return checkWrapperResult(result);
}
} // namespace esotereel