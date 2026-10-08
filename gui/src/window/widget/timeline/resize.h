#pragma once

#include <cstdint>
#include <qpoint.h>
#include <set>

namespace esotereel::window {

enum class ClipResizeEdge : uint8_t { Left, Right };

// 複数選択中のクリップを掴んで伸縮したときの挙動。
enum class ResizeMultiMode {
    GrabbedOnly, // 掴んだクリップだけ
    AllSelected, // 選択中の全クリップに同じ delta
};

// TODO: 設定から読むようにする
constexpr ResizeMultiMode RESIZE_MULTI_MODE = ResizeMultiMode::AllSelected;

constexpr double RESIZE_HANDLE_WIDTH = 6.0; // クリップ端のつかみ判定(px)
constexpr int64_t MIN_CLIP_DURATION = 1;    // frames

struct ClipResizeHit {
    uint64_t clipId;
    ClipResizeEdge edge;
};

struct DragClipResize {
    uint64_t grabbedClipId;
    ClipResizeEdge edge;
    int64_t srcFrame;
    int64_t curFrame;
    bool isWrong;
};

} // namespace esotereel::window
