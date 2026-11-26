#include "ProjectFiles.h"
#include "ClientState.h"
#include "StringView.h"

namespace esotereel {

ProjectInspection::ProjectInspection(esotereel_gui_helper::CProjectInspection ffi)
    : path(OwnedString::toQString(ffi.path)), projectId(OwnedString::toQString(ffi.project_id)),
      versionMajor(ffi.version_major), versionMinor(ffi.version_minor),
      versionRevision(ffi.version_revision), status(ffi.status) {
    OwnedString::free(ffi.path);
    OwnedString::free(ffi.project_id);
}

ProjectOpened::ProjectOpened(esotereel_gui_helper::CProjectOpened ffi)
    : path(OwnedString::toQString(ffi.path)), versionMajor(ffi.version_major),
      versionMinor(ffi.version_minor), versionRevision(ffi.version_revision), status(ffi.status) {
    OwnedString::free(ffi.path);
}

AutosaveGeneration::AutosaveGeneration(esotereel_gui_helper::CAutosaveGeneration ffi)
    : id(OwnedString::toQString(ffi.id)), projectId(OwnedString::toQString(ffi.project_id)),
      createdAtMs(ffi.created_at_ms), sizeBytes(ffi.size_bytes), fileCount(ffi.file_count) {
    OwnedString::free(ffi.id);
    OwnedString::free(ffi.project_id);
}

Result<std::optional<ProjectInspection>> ProjectFiles::takeInspection(ClientState *state) {
    if (!state) {
        return Result<std::optional<ProjectInspection>>::err("State is null");
    }
    auto result = esotereel_gui_helper::client_take_project_inspection(*state);
    return Result<std::optional<ProjectInspection>>(
        result, [](const esotereel_gui_helper::CProjectInspection &ffi) {
            return ProjectInspection(ffi);
        });
}

Result<std::optional<ProjectOpened>> ProjectFiles::takeOpened(ClientState *state) {
    if (!state) {
        return Result<std::optional<ProjectOpened>>::err("State is null");
    }
    auto result = esotereel_gui_helper::client_take_project_opened(*state);
    return Result<std::optional<ProjectOpened>>(
        result, [](const esotereel_gui_helper::CProjectOpened &ffi) {
            return ProjectOpened(ffi);
        });
}

Result<std::optional<QString>> ProjectFiles::takeSaved(ClientState *state) {
    if (!state) {
        return Result<std::optional<QString>>::err("State is null");
    }
    auto result = esotereel_gui_helper::client_take_project_saved(*state);
    return Result<std::optional<QString>>(
        result, [](const esotereel_gui_helper::FfiOwnedString &ffi) {
            QString text = OwnedString::toQString(ffi);
            OwnedString::free(ffi);
            return text;
        });
}

Result<QVector<AutosaveGeneration>> ProjectFiles::takeGenerations(ClientState *state) {
    if (!state) {
        return Result<QVector<AutosaveGeneration>>::err("State is null");
    }
    auto result = esotereel_gui_helper::client_take_autosave_generations(*state);
    return Result<QVector<AutosaveGeneration>>(
        result, [](const esotereel_gui_helper::CAutosaveGeneration &ffi) {
            return AutosaveGeneration(ffi);
        });
}

Result<bool> ProjectFiles::previousSessionAbnormal(ClientState *state) {
    if (!state) {
        return Result<bool>::err("State is null");
    }
    auto result = esotereel_gui_helper::client_previous_session_abnormal(*state);
    return Result<bool>(result);
}

Result<std::optional<QString>> ProjectFiles::currentProjectFolder(ClientState *state) {
    if (!state) {
        return Result<std::optional<QString>>::err("State is null");
    }
    auto result = esotereel_gui_helper::client_current_project_folder(*state);
    return Result<std::optional<QString>>(
        result, [](const esotereel_gui_helper::FfiOwnedString &ffi) {
            QString text = OwnedString::toQString(ffi);
            OwnedString::free(ffi);
            return text;
        });
}

void ProjectFiles::markCleanExit(ClientState *state) {
    if (!state) {
        return;
    }
    esotereel_gui_helper::client_mark_clean_exit(*state);
}

} // namespace esotereel
