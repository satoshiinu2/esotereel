#pragma once

#include "esotereel_gui_helper.h"
#include "ffi/Result.h"
#include "ffi/StringView.h"
#include <QString>
#include <QStringList>
#include <QVector>

namespace esotereel {
class ClientNetworkHandler;

using FfiToolbarButton = esotereel_gui_helper::FfiToolbarButton;

class ToolbarButton {
  public:
    QString id;
    QString label;
    QString tooltip;
    QString icon;
    // Builtin: コマンド名 / Script: "plugin_id::entry"
    QString actionValue;

    ToolbarButton(FfiToolbarButton ffi)
        : id(OwnedString::toQString(ffi.id)), label(OwnedString::toQString(ffi.label)),
          tooltip(OwnedString::toQString(ffi.tooltip)), icon(OwnedString::toQString(ffi.icon)),
          actionValue(OwnedString::toQString(ffi.action_value)) {

        OwnedString::free(ffi.id);
        OwnedString::free(ffi.label);
        OwnedString::free(ffi.tooltip);
        OwnedString::free(ffi.icon);
        OwnedString::free(ffi.action_value);
    }
};

class Toolbar {
  public:
    static Result<QVector<ToolbarButton>> getButtons(ClientNetworkHandler *network, const QString &target);
    static Result<void> setLayout(ClientNetworkHandler *network, const QString &target, const QStringList &orderedIds);
};

} // namespace esotereel
