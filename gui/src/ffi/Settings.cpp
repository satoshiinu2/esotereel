#include "Settings.h"
#include "ClientNetworkHandler.h"
#include "StringView.h"
#include "WrapperResult.h"

#include "esotereel_gui_helper.h"
#include "ffi/ClientNetworkHandler.h"

namespace esotereel {

Result<void> Settings::initialize(ClientNetworkHandler *network, const QString &schemaText) {
    if (!network) {
        return Result<void>::error("Network handler is null");
    }

    QByteArray schemaUtf8 = schemaText.toUtf8();
    RawStringView schema = StringView::fromQUtf8String(schemaUtf8);

    WrapperErrorCode result = esotereel_gui_helper::settings_initialize(*network, schema);
    return wrapperResultToResultVoid(result);
}

Result<QVector<SettingsField>> Settings::getAllFields(ClientNetworkHandler *network) {
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

Result<QString> Settings::getValue(ClientNetworkHandler *network, const QString &key) {
    if (!network) {
        return Result<QString>::error("Network handler is null");
    }

    QByteArray keyUtf8 = key.toUtf8();
    RawStringView keyView = StringView::fromQUtf8String(keyUtf8);
    RawOwnedString output;

    WrapperErrorCode result = esotereel_gui_helper::settings_get_value(*network, keyView, &output);
    if (result != WrapperErrorCode::Ok) {
        return wrapperResultToResult<QString>(result, QString());
    }

    QString value = OwnedString::toQString(output);
    OwnedString::free(output);
    return Result<QString>::ok(value);
}

Result<void> Settings::setValue(ClientNetworkHandler *network, const QString &key, const QString &value) {
    if (!network) {
        return Result<void>::error("Network handler is null");
    }

    QByteArray keyUtf8 = key.toUtf8();
    QByteArray valueUtf8 = value.toUtf8();

    RawStringView keyView = StringView::fromQUtf8String(keyUtf8);
    RawStringView valueView = StringView::fromQUtf8String(valueUtf8);

    WrapperErrorCode result = esotereel_gui_helper::settings_set_value(*network, keyView, valueView);
    return wrapperResultToResultVoid(result);
}

Result<QStringList> Settings::getCategories(ClientNetworkHandler *network) {
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
