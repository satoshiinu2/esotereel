#pragma once

#include "Result.h"
#include "esotereel_gui_helper.h"
#include <QString>
#include <QVector>
#include <cstdint>

namespace esotereel {
class ClientState;

// アプリ設定（階層 TOML）の宣言済みキー1件分、ある層から見た状態。
// scope: 0=UserOnly、1=ProjectOrServerOnly、2=Overridable。
// kind: 0=Int、1=UInt、2=Bool。
struct AppSettingEntry {
    QString key;
    uint8_t scope = 0;
    uint8_t kind = 0;
    bool editableHere = false;
    bool isSet = false;
    QString layerValue;
    QString effectiveValue;
    QString defaultValue;

    AppSettingEntry() = default;
    explicit AppSettingEntry(esotereel_gui_helper::CAppSetting ffi);
};

class AppSettingsFiles {
  public:
    // layer: 0=ユーザー設定、1=プロジェクト設定。
    static Result<QVector<AppSettingEntry>> list(ClientState *state, uint8_t layer);
    static Result<void> setValue(ClientState *state, uint8_t layer, const QString &key,
                                 const QString &value);
    static Result<void> reset(ClientState *state, uint8_t layer, const QString &key);
};

} // namespace esotereel
