#pragma once

#include "esotereel_gui_helper.h"
#include "ffi/FieldValue.h"
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
    FieldValue defaultValue;

    SettingsField() = default;

    SettingsField(esotereel_gui_helper::SettingsField ffi)
        : key(OwnedString::toQString(ffi.key)), category(OwnedString::toQString(ffi.category)),
          label(OwnedString::toQString(ffi.label)), kindType(ffi.kind_type),
          defaultValue(FieldValue::fromC(ffi.default_value)) {

        // Free the owned strings
        OwnedString::free(ffi.key);
        OwnedString::free(ffi.category);
        OwnedString::free(ffi.label);
        // Note: default_value is a CFieldValue which may contain owned strings that need to be freed
        // This is handled in the fromC conversion
    }
};

class Settings {
  public:
    static Result<QVector<SettingsField>> getAllFields(ClientState *network);

    // FieldValue-based accessors (via cxx-qt / QVariant)
    static Result<FieldValue> getValue(ClientState *network, const QString &key);
    static Result<void> setValue(ClientState *network, const QString &key, const FieldValue &value);

    // String-based convenience overloads
    static Result<QString> getValueString(ClientState *network, const QString &key);
    static Result<void> setValue(ClientState *network, const QString &key, const QString &value);
    static Result<void> setValueString(ClientState *network, const QString &key, const QString &value);

    static Result<QStringList> getCategories(ClientState *network);
};

} // namespace esotereel

Q_DECLARE_METATYPE(esotereel::SettingsField)
