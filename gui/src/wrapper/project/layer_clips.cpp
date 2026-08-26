#include "layer_clips.h"
#include "../exception.h"
#include "clip.h"
#include "esotereel_gui_helper.h"
#include "timeline.h"
#include <cstdint>

using RawLayer = esotereel_gui_helper::Layer;
using RawClip = esotereel_gui_helper::Clip;
using RawTimeline = esotereel_gui_helper::Timeline;

// Index-based iterator implementation
ClipsIterator::ClipsIterator(const RawLayer *layer, const RawTimeline *timeline) noexcept
    : raw_layer_ptr(layer), timeline_ptr(timeline), raw_cur_ptr(nullptr), current_index(0), total_count(0) {
    if (raw_layer_ptr && timeline_ptr) {
        total_count = esotereel_gui_helper::layer_get_clips_count(raw_layer_ptr);
        initialize();
    }
}

ClipsIterator::ClipsIterator(const RawLayer *layer, const RawTimeline *timeline, int64_t startFrame, int64_t endFrame) noexcept
    : raw_layer_ptr(layer), timeline_ptr(timeline), raw_cur_ptr(nullptr), current_index(0), total_count(0) {
    if (raw_layer_ptr && timeline_ptr) {
        total_count = esotereel_gui_helper::layer_get_clips_count(raw_layer_ptr);
        // For range-based iteration, we could filter by position here
        // For now, initialize with first clip and let caller filter
        initialize();
    }
}

void ClipsIterator::advance() noexcept {
    if (current_index < total_count) {
        current_index++;
        // Update current clip pointer
        if (current_index < total_count) {
            auto result = esotereel_gui_helper::layer_get_clip_at_index(
                raw_layer_ptr, timeline_ptr, current_index, &raw_cur_ptr);
            if (!checkWrapperResult(result)) {
                raw_cur_ptr = nullptr;
                current_index = total_count; // Stop iteration
            }
        } else {
            raw_cur_ptr = nullptr;
        }
    }
}

// Initialize first clip on construction
void ClipsIterator::initialize() noexcept {
    if (total_count > 0) {
        auto result = esotereel_gui_helper::layer_get_clip_at_index(
            raw_layer_ptr, timeline_ptr, 0, &raw_cur_ptr);
        if (!checkWrapperResult(result)) {
            raw_cur_ptr = nullptr;
            current_index = total_count;
        }
    }
}

// デストラクタ
ClipsIterator::~ClipsIterator() {
    // No dynamic resources to clean up with index-based approach
}

size_t ClipsIterable::clipsCount() const noexcept {
    return raw_ptr ? esotereel_gui_helper::layer_get_clips_count(raw_ptr) : 0;
}

ClipsIterable::ClipsIterable(const RawLayer *p, const Timeline &timeline) noexcept 
    : raw_ptr(p), timeline_ptr(timeline.raw_ptr) {}
