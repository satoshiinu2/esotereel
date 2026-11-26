#include "AppSettingsFiles.h"
#include "ClientState.h"
#include "StringView.h"

namespace esotereel {

AppSettingEntry::AppSettingEntry(esotereel_gui_helper::CAppSetting ffi)
    : key(OwnedString::toQString(ffi.key)), scope(ffi.scope), kind(ffi.kind),
      editableHere(ffi.editable_here), isSet(ffi.is_set),
      layerValue(OwnedString::toQString(ffi.layer_value)),
      effectiveValue(OwnedString::toQString(ffi.effective_value)),
      defaultValue(OwnedString::toQString(ffi.default_value)) {
    OwnedString::free(ffi.key);
    OwnedString::free(ffi.layer_value);
    OwnedString::free(ffi.effective_value);
    OwnedString::free(ffi.default_value);
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
                                        const QString &value) {
    if (!state) {
        return Result<void>::err("State is null");
    }
    QByteArray keyUtf8 = key.toUtf8();
    QByteArray valueUtf8 = value.toUtf8();
    auto result = esotereel_gui_helper::app_settings_set(
        *state, layer, StringView::fromQUtf8String(keyUtf8), StringView::fromQUtf8String(valueUtf8));
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
