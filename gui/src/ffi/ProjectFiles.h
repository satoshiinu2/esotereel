#pragma once

#include "Result.h"
#include "esotereel_gui_helper.h"
#include <QString>
#include <QVector>
#include <cstdint>
#include <optional>

namespace esotereel {
class ClientState;

// プロジェクトフォルダの事前確認結果。status は
// 0=Compatible、1=NewerRevision、2=NewerMinor、3=NewerMajor。
struct ProjectInspection {
    QString path;
    QString projectId;
    uint32_t versionMajor = 0;
    uint32_t versionMinor = 0;
    uint32_t versionRevision = 0;
    uint8_t status = 0;

    ProjectInspection() = default;
    explicit ProjectInspection(esotereel_gui_helper::CProjectInspection ffi);
};

// プロジェクトを開いた結果。
struct ProjectOpened {
    QString path;
    uint32_t versionMajor = 0;
    uint32_t versionMinor = 0;
    uint32_t versionRevision = 0;
    uint8_t status = 0;

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
