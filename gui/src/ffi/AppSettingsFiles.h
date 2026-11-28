#pragma once

#include "Result.h"
#include "esotereel_gui_helper.h"
#include "ffi/FieldValue.h"
#include <QString>
#include <QVector>
#include <cstdint>
#include <optional>

namespace esotereel {
class ClientState;

using CSettingScope = esotereel_gui_helper::CSettingScope;
using SettingsFieldType = esotereel_gui_helper::SettingsFieldType;

// アプリ設定（階層 TOML）の宣言済みキー1件分、ある層から見た状態。
// 値はクリッププロパティと同じ FieldValue で保持します（未設定は nullopt）。
struct AppSettingEntry {
    QString key;
    CSettingScope scope = CSettingScope::UserOnly;
    bool editableHere = false;
    bool isSet = false;
    std::optional<FieldValue> layerValue;
    std::optional<FieldValue> effectiveValue;
    std::optional<FieldValue> defaultValue;
    // FieldControlFactory 用の編集種別。値のタグから決めます。
    // どの層にも値が無いキーは Int として扱います。
    SettingsFieldType kindType = SettingsFieldType::Int;

    AppSettingEntry() = default;
    explicit AppSettingEntry(esotereel_gui_helper::CAppSetting ffi);
};

class AppSettingsFiles {
  public:
    // layer: 0=ユーザー設定、1=プロジェクト設定。
    static Result<QVector<AppSettingEntry>> list(ClientState *state, uint8_t layer);
    static Result<void> setValue(ClientState *state, uint8_t layer, const QString &key,
                                 const FieldValue &value);
    static Result<void> reset(ClientState *state, uint8_t layer, const QString &key);
};

} // namespace esotereel
