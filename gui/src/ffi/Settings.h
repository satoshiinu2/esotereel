#pragma once

#include "esotereel_gui_helper.h"
#include "ffi/Result.h"
#include "ffi/StringView.h"
#include <QString>
#include <QStringList>
#include <QVector>

namespace esotereel {
class ClientState;

using SettingsFieldType = esotereel_gui_helper::SettingsFieldType;

class SettingsField {
  public:
    QString key;
    QString category;
    QString label;
    SettingsFieldType kindType;
    QString defaultValue;

    SettingsField(esotereel_gui_helper::SettingsField ffi)
        : key(OwnedString::toQString(ffi.key)), category(OwnedString::toQString(ffi.category)),
          label(OwnedString::toQString(ffi.label)), kindType(ffi.kind_type),
          defaultValue(OwnedString::toQString(ffi.default_value)) {

        // Free the owned strings
        OwnedString::free(ffi.key);
        OwnedString::free(ffi.category);
        OwnedString::free(ffi.label);
        OwnedString::free(ffi.default_value);
    }
};

class Settings {
  public:
    static Result<QVector<SettingsField>> getAllFields(ClientState *network);
    static Result<QString> getValue(ClientState *network, const QString &key);
    static Result<void> setValue(ClientState *network, const QString &key, const QString &value);
    static Result<QStringList> getCategories(ClientState *network);
};

} // namespace esotereel
