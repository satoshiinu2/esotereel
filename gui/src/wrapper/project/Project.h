#pragma once

#include "wrapper/Result.h"
#include <cmath>
#include <cstddef>
#include <utility>

// C-FFI (esotereel_gui_helper) 側の型のプロトタイプ宣言
namespace esotereel_gui_helper {
struct Project;
struct ClientNetworkHandler;
struct WrapperResult;
} // namespace esotereel_gui_helper

namespace esotereel {
using RawProject = esotereel_gui_helper::Project;

class Timeline;
class ClientNetworkHandler; // C++ ラッパークラスの前方宣言

class Project {
    const void *guard_ptr = nullptr;
    const RawProject *project_ptr = nullptr;

  public:
    Project(const void *g, const RawProject *p);
    ~Project();

    // コピー禁止（二重ドロップ/アンロックを確実に防止）
    Project(const Project &) = delete;
    Project &operator=(const Project &) = delete;

    // ムーブコンストラクタ & ムーブ代入演算子
    Project(Project &&other) noexcept;
    Project &operator=(Project &&other) noexcept;

    operator const RawProject *() const noexcept {
        return project_ptr;
    }

    // C++ の ClientNetworkHandler インスタンスから 1 行で Project ロックを取得する静的関数
    static Result<Project> lockRead(const ClientNetworkHandler *network);

    static Result<Project> byGuard(const void *guard_ptr);
    static Project invalid();

    bool isValid() const noexcept;
    Timeline timelineOf(size_t index) const noexcept;
    size_t timelineCount() const noexcept;
    void debugLog() const noexcept;
};
} // namespace esotereel