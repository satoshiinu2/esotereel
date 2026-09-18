#include "Settings.h"
#include "ClientState.h"
#include "StringView.h"
#include "WrapperResult.h"

#include "esotereel_gui_helper.h"
#include "ffi/ClientState.h"

namespace esotereel {

Result<QVector<SettingsField>> Settings::getAllFields(ClientState *network) {
    if (!network) {
        return Result<QVector<SettingsField>>::err("Network handler is null");
    }

    QVector<SettingsField> fields;

    // Get count first
    int32_t count = esotereel_gui_helper::settings_get_all_fields_count(*network);
    if (count < 0) {
        return Result<QVector<SettingsField>>::err("Failed to get fields count");
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
        return Result<FieldValue>::err("Network handler is null");
    }

    QByteArray keyUtf8 = key.toUtf8();
    RawStringView keyView = StringView::fromQUtf8String(keyUtf8);

    auto result = esotereel_gui_helper::settings_get_value(*network, keyView);

    if (!result.is_ok) {
        return Result<FieldValue>::err(OwnedString::intoStdString(result.value.err));
    }

    // Convert CFieldValue to FieldValue
    if (result.value.ok.has_value) {
        return Result<FieldValue>::ok(FieldValue::fromC(result.value.ok.value.some));
    } else {
        return Result<FieldValue>::err("Setting not found");
    }
}

Result<void> Settings::setValue(ClientState *network, const QString &key, const FieldValue &value) {
    if (!network) {
        return Result<void>::err("Network handler is null");
    }

    // Convert FieldValue to CFieldValue
    // This is complex because we need to allocate memory in Rust and convert the variant
    // For now, return error until we implement the full conversion
    return Result<void>::err("Setting value from FieldValue not yet implemented - need CFieldValue conversion");
}

Result<QString> Settings::getValueString(ClientState *network, const QString &key) {
    auto res = getValue(network, key);
    if (res.is_ok()) {
        // Convert FieldValue to string
        if (std::holds_alternative<std::string>(res.unwrap().variant())) {
            return Result<QString>::ok(QString::fromStdString(std::get<std::string>(res.unwrap().variant())));
        }
        return Result<QString>::err("Value is not a string");
    }
    return Result<QString>::err(res.error());
}

Result<void> Settings::setValue(ClientState *network, const QString &key, const QString &value) {
    // Temporarily disabled until CFieldValue conversion is implemented
    return Result<void>::err("Setting value not yet implemented");
}

Result<void> Settings::setValueString(ClientState *network, const QString &key, const QString &value) {
    // Temporarily disabled until CFieldValue conversion is implemented
    return Result<void>::err("Setting value not yet implemented");
}

Result<QStringList> Settings::getCategories(ClientState *network) {
    if (!network) {
        return Result<QStringList>::err("Network handler is null");
    }

    QStringList categories;

    // Get count first
    int32_t count = esotereel_gui_helper::settings_get_categories_count(*network);
    if (count < 0) {
        return Result<QStringList>::err("Failed to get categories count");
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
