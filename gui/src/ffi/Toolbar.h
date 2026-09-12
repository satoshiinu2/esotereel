#pragma once

#include "esotereel_gui_helper.h"
#include "ffi/Result.h"
#include "ffi/StringView.h"
#include <QString>
#include <QStringList>
#include <QVector>
#include <qobject.h>

namespace esotereel {
class ClientState;

using FfiToolbarButton = esotereel_gui_helper::FfiToolbarButton;

class ToolbarButton {
  public:
    // {plugin_id}.{button_id}
    QString id;
    QString label;
    QString tooltip;
    QString icon;
    QString action;

    ToolbarButton(FfiToolbarButton ffi)
        : id(OwnedString::toQString(ffi.id)), label(OwnedString::toQString(ffi.label)),
          tooltip(OwnedString::toQString(ffi.tooltip)), icon(OwnedString::toQString(ffi.icon)),
          action(OwnedString::toQString(ffi.action)) {

        OwnedString::free(ffi.id);
        OwnedString::free(ffi.label);
        OwnedString::free(ffi.tooltip);
        OwnedString::free(ffi.icon);
        OwnedString::free(ffi.action);
    }

    Result<void> handleAction(ClientState *network);
};

class Toolbar {
  public:
    static Result<QVector<ToolbarButton>> getButtons(ClientState *network, const QString &target);
    static Result<void> setLayout(ClientState *network, const QString &target, const QStringList &orderedIds);
};

} // namespace esotereel
