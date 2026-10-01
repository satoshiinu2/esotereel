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
using TimelineId = esotereel_gui_helper::TimelineId;

class ToolbarButton {
  public:
    // {plugin_id}.{button_id}
    QString id;
    QString label;
    QString tooltip;
    QString icon;
    QString action;
    QString runOn;

    ToolbarButton(FfiToolbarButton ffi)
        : id(OwnedString::toQString(ffi.id)), label(OwnedString::toQString(ffi.label)),
          tooltip(OwnedString::toQString(ffi.tooltip)), icon(OwnedString::toQString(ffi.icon)),
          action(OwnedString::toQString(ffi.action)), runOn(OwnedString::toQString(ffi.run_on)) {

        OwnedString::free(ffi.id);
        OwnedString::free(ffi.label);
        OwnedString::free(ffi.tooltip);
        OwnedString::free(ffi.icon);
        OwnedString::free(ffi.action);
        OwnedString::free(ffi.run_on);
    }

    Result<void> handleAction(ClientState *state, TimelineId timelineId);
};

class Toolbar {
  public:
    static Result<QVector<ToolbarButton>> getButtons(ClientState *state, const QString &target);
    static Result<void> setLayout(ClientState *state, const QString &target, const QStringList &orderedIds);
};

} // namespace esotereel
