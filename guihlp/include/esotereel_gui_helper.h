#pragma once

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

namespace esotereel_gui_helper {

struct _Clip;

struct _ClipIterator;

struct _Layer;

struct _Project;

struct _Timeline;

struct _WGpuUtil;

struct _GuiCallbacks {
  void (*on_test)();
  void (*on_update_timeline)(uintptr_t timeline_type);
};

struct _StringView {
  const uint8_t *ptr;
  uintptr_t len;
};

using _OnSendFn = void(*)(const uint8_t*, uintptr_t);

using _LogOutCStrFn = void(*)(uintptr_t level, const uint8_t *ptr, uintptr_t len);

struct _ClipLocation {
  uintptr_t layer_idx;
  uintptr_t clip_idx;
  const _Clip *clip;
};

extern "C" {

void init();

void set_gui_callbacks(_GuiCallbacks callbacks);

_StringView parse_responce(const uint8_t *ptr, uintptr_t len);

void set_send_callback(_OnSendFn callback);

const _Project *get_project();

void req_clip_move_mul(uintptr_t timeline_idx,
                       const uint64_t *ptr,
                       uintptr_t len,
                       int64_t position_moved,
                       int64_t duration_added,
                       intptr_t layer_moved);

void req_add_clip_dummy(uintptr_t timeline_idx, int64_t position, uintptr_t layer_idx);

void init_rust_logger(_LogOutCStrFn callback);

const _Timeline *project_get_timeline(const _Project *ptr, uintptr_t id);

uintptr_t project_get_timeline_count(const _Project *ptr);

uint64_t clip_get_id(const _Clip *ptr);

int64_t clip_get_position(const _Clip *ptr);

int64_t clip_get_duration(const _Clip *ptr);

_ClipLocation layer_find_clip_at_frame(const _Layer *ptr, int64_t frame, uintptr_t layer_idx);

const _Clip *layer_get_clip_at_slow(const _Layer *ptr, uintptr_t idx);

uintptr_t layer_get_clips_count(const _Layer *ptr);

_StringView layer_get_name(const _Layer *ptr);

_ClipIterator *layer_clips_begin(const _Layer *layer);

const _Clip *clip_iter_next(_ClipIterator *iter);

void clip_iter_free(_ClipIterator *iter);

const _Layer *timeline_get_layer_at(const _Timeline *ptr, uintptr_t l_idx);

uintptr_t timeline_get_layers_count(const _Timeline *ptr);

_ClipLocation timeline_find_clip_by_id(const _Timeline *ptr, uint64_t clip_id);

bool timeline_can_place_clip_at(const _Timeline *ptr,
                                uintptr_t layer_idx,
                                int64_t position,
                                int64_t duration,
                                const uint64_t *exclude_ids_ptr,
                                uintptr_t exclude_ids_len);

_WGpuUtil *wgpuutil_init_surface(void *window_ptr,
                                 void *display_ptr,
                                 uint32_t width,
                                 uint32_t height,
                                 bool is_wayland);

void wgpuutil_drop(_WGpuUtil *ptr);

void wgpuutil_update_surface(_WGpuUtil *ptr, void *window_ptr, void *display_ptr, bool is_wayland);

void wgpuutil_update_size(_WGpuUtil *ptr, uint32_t width, uint32_t height);

void render_frame(_WGpuUtil *ptr);

void req_test();

void req_new_project();

}  // extern "C"

}  // namespace esotereel_gui_helper
