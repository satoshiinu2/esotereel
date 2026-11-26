#pragma once

#include <QString>
#include <array>
#include <cstdint>
#include <optional>

#include "ClientState.h"
#include "esotereel_gui_helper.h"

namespace esotereel {
using TimelineId = esotereel_gui_helper::TimelineId;
using LayerId = esotereel_gui_helper::LayerId;
using LayerFolderId = esotereel_gui_helper::LayerFolderId;
using ClipId = esotereel_gui_helper::ClipId;
using TimelineTick = esotereel_gui_helper::TimelineTick;
using OptionProject = esotereel_gui_helper::OptionProject;

class Requests {
    const esotereel_gui_helper::ClientStateHandle *ptr_network;

  public:
    explicit Requests(const ClientState *network);
    void newProject();
    void inspectProject(const QString &path);
    void openProject(const QString &path, bool backup);
    void saveProject(bool overwrite);
    void saveProjectAs(const QString &path, bool overwrite);
    void listAutosaveGenerations();
    void restoreAutosaveGeneration(const QString &generationId);
    // アセット ID は UUID の16バイトを文字列化せずそのまま渡す。
    // ファイルを新規アセットとして登録し、採番された参照（UUID の16バイト）を返す。失敗時は nullopt。
    std::optional<std::array<uint8_t, 16>> registerAssetReference(const QString &path);
    void renameAsset(const std::array<uint8_t, 16> &assetId, const QString &name);
    void removeAsset(const std::array<uint8_t, 16> &assetId);
    void relinkAsset(const std::array<uint8_t, 16> &assetId, const QString &path);
    void loadStream(QString path) noexcept;
    void fetchFrame(TimelineId timelineIdx, TimelineTick playhead,
                    std::pair<TimelineTick, TimelineTick> visible_range) noexcept;

    void debugProjectLog() noexcept;
};
} // namespace esotereel