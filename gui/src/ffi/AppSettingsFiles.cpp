#include "AppSettingsFiles.h"
#include "ClientState.h"
#include "StringView.h"

namespace esotereel {
namespace {

std::optional<FieldValue> convertValue(const FfiOption<CFieldValue> &ffi) {
    if (!ffi.has_value) {
        return std::nullopt;
    }
    return FieldValue::fromC(ffi.value.some);
}

SettingsFieldType deriveKindType(const std::optional<FieldValue> &layerValue,
                                 const std::optional<FieldValue> &effectiveValue,
                                 const std::optional<FieldValue> &defaultValue) {
    for (const auto *value : {&layerValue, &effectiveValue, &defaultValue}) {
        if (!value->has_value()) {
            continue;
        }
        const FieldValue::Variant &variant = (*value)->variant();
        if (std::holds_alternative<bool>(variant)) {
            return SettingsFieldType::Bool;
        }
        if (std::holds_alternative<std::int64_t>(variant)) {
            return SettingsFieldType::Int;
        }
        if (std::holds_alternative<double>(variant)) {
            return SettingsFieldType::Float;
        }
        if (std::holds_alternative<EnumValue>(variant)) {
            return SettingsFieldType::Enum;
        }
        if (std::holds_alternative<std::string>(variant)) {
            return SettingsFieldType::String;
        }
    }
    return SettingsFieldType::Int;
}

} // namespace

AppSettingEntry::AppSettingEntry(esotereel_gui_helper::CAppSetting ffi)
    : key(OwnedString::toQString(ffi.key)), scope(ffi.scope), editableHere(ffi.editable_here),
      isSet(ffi.is_set), layerValue(convertValue(ffi.layer_value)),
      effectiveValue(convertValue(ffi.effective_value)),
      defaultValue(convertValue(ffi.default_value)),
      kindType(deriveKindType(layerValue, effectiveValue, defaultValue)) {
    OwnedString::free(ffi.key);
}

Result<QVector<AppSettingEntry>> AppSettingsFiles::list(ClientState *state, uint8_t layer) {
    if (!state) {
        return Result<QVector<AppSettingEntry>>::err("State is null");
    }
    auto result = esotereel_gui_helper::app_settings_list(*state, layer);
    return Result<QVector<AppSettingEntry>>(
        result, [](const esotereel_gui_helper::CAppSetting &ffi) { return AppSettingEntry(ffi); });
}

Result<void> AppSettingsFiles::setValue(ClientState *state, uint8_t layer, const QString &key,
                                        const FieldValue &value) {
    if (!state) {
        return Result<void>::err("State is null");
    }
    QByteArray keyUtf8 = key.toUtf8();
    CFieldValue cValue = value.toC();
    auto result = esotereel_gui_helper::app_settings_set(
        *state, layer, StringView::fromQUtf8String(keyUtf8), cValue);
    // toC()で自前確保したバッファを解放（Rustが作るCFieldValueの解放とは別ルート）。
    FieldValue::freeC(cValue);
    return Result<void>(result);
}

Result<void> AppSettingsFiles::reset(ClientState *state, uint8_t layer, const QString &key) {
    if (!state) {
        return Result<void>::err("State is null");
    }
    QByteArray keyUtf8 = key.toUtf8();
    auto result =
        esotereel_gui_helper::app_settings_reset(*state, layer, StringView::fromQUtf8String(keyUtf8));
    return Result<void>(result);
}

} // namespace esotereel
