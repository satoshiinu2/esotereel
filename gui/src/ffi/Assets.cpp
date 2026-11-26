#include "Assets.h"
#include "ClientState.h"
#include "StringView.h"
#include <algorithm>

namespace esotereel {

AssetEntry::AssetEntry(esotereel_gui_helper::CAssetEntry ffi)
    : name(OwnedString::toQString(ffi.name)), availability(ffi.availability),
      path(OwnedString::toQString(ffi.path)), hasFingerprintHash(ffi.has_fingerprint_hash),
      inUse(ffi.in_use) {
    std::copy(std::begin(ffi.id.bytes), std::end(ffi.id.bytes), id.begin());
    if (ffi.fingerprint_size.has_value) {
        fingerprintSize = ffi.fingerprint_size.value.some;
    }
    OwnedString::free(ffi.name);
    OwnedString::free(ffi.path);
}

Result<QVector<AssetEntry>> Assets::list(ClientState *state) {
    if (!state) {
        return Result<QVector<AssetEntry>>::err("State is null");
    }
    auto result = esotereel_gui_helper::client_synced_assets(*state);
    return Result<QVector<AssetEntry>>(
        result, [](const esotereel_gui_helper::CAssetEntry &ffi) { return AssetEntry(ffi); });
}

} // namespace esotereel
