#include "Settings.h"
#include "ClientState.h"
#include "StringView.h"
#include "WrapperResult.h"

#include "esotereel_gui_helper.h"
#include "ffi/ClientState.h"

namespace esotereel {

Result<QVector<SettingsField>> Settings::getAllFields(ClientState *network) {
    if (!network) {
        return Result<QVector<SettingsField>>::error("Network handler is null");
    }

    QVector<SettingsField> fields;

    // Get count first
    int32_t count = esotereel_gui_helper::settings_get_all_fields_count(*network);
    if (count < 0) {
        return Result<QVector<SettingsField>>::error("Failed to get fields count");
    }

    if (count == 0) {
        return Result<QVector<SettingsField>>::ok(fields);
    }

    // Allocate buffer
    QVector<esotereel_gui_helper::SettingsField> ffiFields(count);

    WrapperErrorCode result = esotereel_gui_helper::settings_get_all_fields(*network, ffiFields.data(), count);
    if (result != WrapperErrorCode::Ok) {
        return wrapperResultToResult<QVector<SettingsField>>(result, fields);
    }

    // Convert to Qt types and free memory
    for (int i = 0; i < count; ++i) {
        const auto &ffi = ffiFields[i];

        fields.append(SettingsField(ffi));
    }

    return Result<QVector<SettingsField>>::ok(fields);
}

Result<FieldValue> Settings::getValue(ClientState *network, const QString &key) {
    if (!network) {
        return Result<FieldValue>::error("Network handler is null");
    }

    QVariant variant = esotereel_gui_helper::settings_get_value(*network, &key);

    if (!variant.isValid()) {
        return Result<FieldValue>::error("Failed to get setting value");
    }

    return Result<FieldValue>::ok(FieldValue::fromVariant(variant));
}

Result<void> Settings::setValue(ClientState *network, const QString &key, const FieldValue &value) {
    if (!network) {
        return Result<void>::error("Network handler is null");
    }

    QVariant variant = value.toVariant();

    bool ok = esotereel_gui_helper::settings_set_value(*network, key, &variant);

    if (!ok) {
        return Result<void>::error("Failed to set setting value");
    }

    return Result<void>::ok();
}

Result<QString> Settings::getValueString(ClientState *network, const QString &key) {
    auto res = getValue(network, key);
    if (res.isOk()) {
        return Result<QString>::ok(res.unwrap().asString());
    }
    return Result<QString>::error(res.errorMessage().value_or("Unknown error"));
}

Result<void> Settings::setValue(ClientState *network, const QString &key, const QString &value) {
    return setValue(network, key, FieldValue::fromString(value));
}

Result<void> Settings::setValueString(ClientState *network, const QString &key, const QString &value) {
    return setValue(network, key, FieldValue::fromString(value));
}

Result<QStringList> Settings::getCategories(ClientState *network) {
    if (!network) {
        return Result<QStringList>::error("Network handler is null");
    }

    QStringList categories;

    // Get count first
    int32_t count = esotereel_gui_helper::settings_get_categories_count(*network);
    if (count < 0) {
        return Result<QStringList>::error("Failed to get categories count");
    }

    if (count == 0) {
        return Result<QStringList>::ok(categories);
    }

    // Allocate buffer
    QVector<RawOwnedString> categoryViews(count);

    WrapperErrorCode result = esotereel_gui_helper::settings_get_categories(*network, categoryViews.data(), count);
    if (result != WrapperErrorCode::Ok) {
        return wrapperResultToResult<QStringList>(result, categories);
    }

    // Convert to Qt types and free memory
    for (int i = 0; i < count; ++i) {
        categories.append(OwnedString::toQString(categoryViews[i]));
        OwnedString::free(categoryViews[i]);
    }

    return Result<QStringList>::ok(categories);
}
} // namespace esotereel
