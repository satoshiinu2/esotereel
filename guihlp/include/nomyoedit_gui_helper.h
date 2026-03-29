#pragma once

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

namespace nomyoedit_gui_helper {

struct Clip;

struct ClipIterator;

struct Layer;

struct Project;

struct Timeline;

struct WGpuUtil;

struct GuiCallbacks {
  void (*on_test)();
  void (*on_update_timeline)(uintptr_t timeline_type);
};

using OnSendFn = void(*)(const uint8_t*, uintptr_t);

using LogOutCStrFn = void(*)(uintptr_t level, const uint8_t *ptr, uintptr_t len);

struct ClipLocation {
  uintptr_t layer_idx;
  uintptr_t clip_idx;
  const Clip *clip;
};

extern "C" {

void init();

void set_gui_callbacks(GuiCallbacks callbacks);

void parse_responce(const uint8_t *ptr, uintptr_t len);

void set_send_callback(OnSendFn callback);

const Project *get_project();

void cmd_test();

void cmd_new_project();

void cmd_clip_move_mul(uintptr_t timeline_type,
                       const uint64_t *ptr,
                       uintptr_t len,
                       int64_t position_moved,
                       int64_t duration_added,
                       intptr_t layer_moved);

void init_rust_logger(LogOutCStrFn callback);

const Timeline *project_get_timeline(const Project *ptr, uintptr_t id);

uintptr_t project_get_timeline_count(const Project *ptr);

uint64_t clip_get_id(const Clip *ptr);

int64_t clip_get_position(const Clip *ptr);

int64_t clip_get_duration(const Clip *ptr);

ClipLocation layer_find_clip_at_frame(const Layer *ptr, int64_t frame, uintptr_t layer_idx);

const Clip *layer_get_clip_at_slow(const Layer *ptr, uintptr_t idx);

uintptr_t layer_get_clips_count(const Layer *ptr);

const uint8_t *layer_get_name_ptr(const Layer *ptr);

uintptr_t layer_get_name_len(const Layer *ptr);

ClipIterator *layer_clips_begin(const Layer *layer);

const Clip *clip_iter_next(ClipIterator *iter);

void clip_iter_free(ClipIterator *iter);

const Layer *timeline_get_layer_at(const Timeline *ptr, uintptr_t l_idx);

uintptr_t timeline_get_layers_count(const Timeline *ptr);

int64_t timeline_get_playhead(const Timeline *ptr);

ClipLocation timeline_find_clip_by_id(const Timeline *ptr, uint64_t clip_id);

bool timeline_can_place_clip_at(const Timeline *ptr,
                                uintptr_t layer_idx,
                                int64_t position,
                                int64_t duration,
                                const uint64_t *exclude_ids_ptr,
                                uintptr_t exclude_ids_len);

WGpuUtil *wgpuutil_init_surface(void *window_ptr,
                                void *display_ptr,
                                uint32_t width,
                                uint32_t height,
                                bool is_wayland);

void wgpuutil_drop(WGpuUtil *ptr);

void wgpuutil_update_surface(WGpuUtil *ptr, void *window_ptr, void *display_ptr, bool is_wayland);

void wgpuutil_update_size(WGpuUtil *ptr, uint32_t width, uint32_t height);

void render_frame(WGpuUtil *ptr);

}  // extern "C"

}  // namespace nomyoedit_gui_helper
