#pragma once

#include "esotereel_gui_helper.h"
#include "ffi/Result.h"
#include "ffi/StringView.h"
#include <QString>
#include <QStringList>
#include <QVector>

namespace esotereel {
class ClientNetworkHandler;

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
    static Result<void> initialize(ClientNetworkHandler *network, const QString &schemaText);
    static Result<QVector<SettingsField>> getAllFields(ClientNetworkHandler *network);
    static Result<QString> getValue(ClientNetworkHandler *network, const QString &key);
    static Result<void> setValue(ClientNetworkHandler *network, const QString &key, const QString &value);
    static Result<QStringList> getCategories(ClientNetworkHandler *network);
};

} // namespace esotereel
