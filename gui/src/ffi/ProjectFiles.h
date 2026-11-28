#pragma once

#include "Result.h"
#include "esotereel_gui_helper.h"
#include <QString>
#include <QVector>
#include <cstdint>
#include <optional>

namespace esotereel {
class ClientState;

using CVersionStatus = esotereel_gui_helper::CVersionStatus;

// プロジェクトフォルダの事前確認結果。
struct ProjectInspection {
    QString path;
    QString projectId;
    uint32_t versionMajor = 0;
    uint32_t versionMinor = 0;
    uint32_t versionRevision = 0;
    CVersionStatus status = CVersionStatus::Compatible;

    ProjectInspection() = default;
    explicit ProjectInspection(esotereel_gui_helper::CProjectInspection ffi);
};

// プロジェクトを開いた結果。
struct ProjectOpened {
    QString path;
    uint32_t versionMajor = 0;
    uint32_t versionMinor = 0;
    uint32_t versionRevision = 0;
    CVersionStatus status = CVersionStatus::Compatible;

    ProjectOpened() = default;
    explicit ProjectOpened(esotereel_gui_helper::CProjectOpened ffi);
};

// オートセーブ世代1件。
struct AutosaveGeneration {
    QString id;
    QString projectId;
    uint64_t createdAtMs = 0;
    uint64_t sizeBytes = 0;
    uint64_t fileCount = 0;

    AutosaveGeneration() = default;
    explicit AutosaveGeneration(esotereel_gui_helper::CAutosaveGeneration ffi);
};

class ProjectFiles {
  public:
    static Result<std::optional<ProjectInspection>> takeInspection(ClientState *state);
    static Result<std::optional<ProjectOpened>> takeOpened(ClientState *state);
    static Result<std::optional<QString>> takeSaved(ClientState *state);
    static Result<QVector<AutosaveGeneration>> takeGenerations(ClientState *state);
    static Result<bool> previousSessionAbnormal(ClientState *state);
    static Result<std::optional<QString>> currentProjectFolder(ClientState *state);
    static void markCleanExit(ClientState *state);
};

} // namespace esotereel
