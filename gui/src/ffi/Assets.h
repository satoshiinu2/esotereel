#pragma once

#include "Result.h"
#include "esotereel_gui_helper.h"
#include <QString>
#include <QVector>
#include <array>
#include <cstdint>
#include <optional>

namespace esotereel {
class ClientState;

// サーバーから同期されたアセット1件。id は UUID の16バイトを
// 文字列化せずそのまま保持する。availability は
// 0=External、1=Managed、2=Missing。path は種別に応じたパス
// （Missing では以前のパス）。fingerprintSize は未計測なら空。
struct AssetEntry {
    std::array<uint8_t, 16> id{};
    QString name;
    uint8_t availability = 0;
    QString path;
    std::optional<uint64_t> fingerprintSize;
    bool hasFingerprintHash = false;
    bool inUse = false;

    AssetEntry() = default;
    explicit AssetEntry(esotereel_gui_helper::CAssetEntry ffi);
};

class Assets {
  public:
    static Result<QVector<AssetEntry>> list(ClientState *state);
};

} // namespace esotereel
