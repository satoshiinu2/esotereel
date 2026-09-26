#include "Settings.h"
#include "ClientState.h"
#include "StringView.h"

#include "esotereel_gui_helper.h"
#include "ffi/ClientState.h"
#include "ffi/Array.h"
#include "ffi/Option.h"

namespace esotereel {

Result<QVector<SettingsField>> Settings::getAllFields(ClientState *state) {
    if (!state) {
        return Result<QVector<SettingsField>>::err("State is null");
    }

    auto result = esotereel_gui_helper::settings_get_all_fields(*state);

    // Use the new Result constructor that handles FfiResult<FfiArray<T>>
    return Result<QVector<SettingsField>>(result, [](const esotereel_gui_helper::FfiPropertySchema &schema) {
        return SettingsField(schema);
    });
}

Result<FieldValue> Settings::getValue(ClientState *state, const QString &key) {
    if (!state) {
        return Result<FieldValue>::err("State is null");
    }

    QByteArray keyUtf8 = key.toUtf8();
    RawStringView keyView = StringView::fromQUtf8String(keyUtf8);

    auto result = esotereel_gui_helper::settings_get_value(*state, keyView);

    if (!result.is_ok) {
        return Result<FieldValue>::err(OwnedString::intoStdString(result.value.err));
    }

    if (Option::isNone(result.value.ok)) {
        return Result<FieldValue>::err("Setting not found");
    }

    return Result<FieldValue>::ok(FieldValue::fromC(Option::unwrap(result.value.ok)));
}

Result<void> Settings::setValue(ClientState *state, const QString &key, const FieldValue &value) {
    if (!state) {
        return Result<void>::err("State is null");
    }

    QByteArray keyUtf8 = key.toUtf8();
    RawStringView keyView = StringView::fromQUtf8String(keyUtf8);

    esotereel_gui_helper::CFieldValue cValue = value.toC();

    auto result = esotereel_gui_helper::settings_set_value(*state, keyView, cValue);

    // toC()で自前確保したバッファを解放(Rustが作るCFieldValueの解放とは別ルート)。
    FieldValue::freeC(cValue);

    if (!result.is_ok) {
        return Result<void>::err(OwnedString::intoStdString(result.err));
    }

    return Result<void>::ok();
}

Result<QString> Settings::getValueString(ClientState *state, const QString &key) {
    auto res = getValue(state, key);
    if (res.isOk()) {
        // Convert FieldValue to string
        if (std::holds_alternative<std::string>(res.unwrap().variant())) {
            return Result<QString>::ok(QString::fromStdString(std::get<std::string>(res.unwrap().variant())));
        }
        return Result<QString>::err("Value is not a string");
    }
    return Result<QString>::err(res.error());
}

Result<void> Settings::setValue(ClientState *state, const QString &key, const QString &value) {
    return setValue(state, key, FieldValue::fromString(value));
}

Result<void> Settings::setValueString(ClientState *state, const QString &key, const QString &value) {
    return setValue(state, key, value);
}

Result<QStringList> Settings::getCategories(ClientState *state) {
    if (!state) {
        return Result<QStringList>::err("State is null");
    }

    auto result = esotereel_gui_helper::settings_get_categories(*state);

    // Use the new Result constructor that handles FfiResult<FfiArray<T>>
    return Result<QStringList>(result, [](const esotereel_gui_helper::FfiOwnedString &owned) {
        return OwnedString::toQString(owned);
    });
}
} // namespace esotereel
