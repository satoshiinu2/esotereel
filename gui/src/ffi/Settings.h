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

    SettingsField(esotereel_gui_helper::FfiPropertySchema ffi)
        : key(OwnedString::toQString(ffi.key)), category(OwnedString::toQString(ffi.category)),
          label(OwnedString::toQString(ffi.label)), kindType(ffi.kind_type),
          defaultValue(FieldValue::fromC(ffi.default_value)) {

        // Free the owned strings (default_value内の所有権はfromCの変換過程で処理済み)
        OwnedString::free(ffi.key);
        OwnedString::free(ffi.category);
        OwnedString::free(ffi.label);
    }
};

class Settings {
  public:
    static Result<QVector<SettingsField>> getAllFields(ClientState *state);

    // FieldValue-based accessors (via cxx-qt / QVariant)
    static Result<FieldValue> getValue(ClientState *state, const QString &key);
    static Result<void> setValue(ClientState *state, const QString &key, const FieldValue &value);

    // String-based convenience overloads
    static Result<QString> getValueString(ClientState *state, const QString &key);
    static Result<void> setValue(ClientState *state, const QString &key, const QString &value);
    static Result<void> setValueString(ClientState *state, const QString &key, const QString &value);

    static Result<QStringList> getCategories(ClientState *state);
};

} // namespace esotereel

Q_DECLARE_METATYPE(esotereel::SettingsField)
