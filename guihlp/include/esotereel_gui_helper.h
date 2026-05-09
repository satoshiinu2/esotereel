#pragma once

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

namespace esotereel_gui_helper {

enum class _WrapperErrorCode {
  Ok = 0,
  NullPtr = 1,
  NotFound = 2,
  Error = 3,
  Panic = 4,
};

struct _ClientNetworkHandler;

struct _Clip;

struct _ClipIterator;

struct _Layer;

struct _Project;

struct _Timeline;

struct _WGpuUtil;

struct _GuiCallbacks {
  void (*on_test)();
  void (*redraw_timeline)(uintptr_t timeline_type);
};

using _OnConnectedFn = void(*)();

struct _StringView {
  const uint8_t *ptr;
  uintptr_t len;
};

using _OnServerReadyFn = void(*)(bool);

using _LogOutCStrFn = void(*)(uintptr_t level, _StringView target, _StringView msg);

extern "C" {

void init();

void set_gui_callbacks(_GuiCallbacks callbacks);

void set_on_connected_callback(_OnConnectedFn callback);

void req_cmd_clip_move_mul(const _ClientNetworkHandler *ptr_network,
                           uintptr_t timeline_idx,
                           const uint64_t *ptr,
                           uintptr_t len,
                           int64_t position_moved,
                           int64_t duration_added,
                           intptr_t layer_moved);

void req_cmd_add_clip_dummy(const _ClientNetworkHandler *ptr_network,
                            uintptr_t timeline_idx,
                            int64_t position,
                            uintptr_t layer_idx);

_WrapperErrorCode internal_server_start(_StringView addr, _OnServerReadyFn on_server_ready);

void init_rust_logger(_LogOutCStrFn callback);

_WrapperErrorCode client_network_handler_run(const _ClientNetworkHandler *ptr, _StringView addr);

_WrapperErrorCode client_network_handler_new(const _ClientNetworkHandler **out);

_WrapperErrorCode client_network_handler_drop(const _ClientNetworkHandler *ptr);

_WrapperErrorCode client_network_handler_app_state_project_lock_read(const _ClientNetworkHandler *ptr,
                                                                     const void **out);

_WrapperErrorCode client_network_handler_app_state_project_unlock_read(const void *guard_ptr);

const _Timeline *project_get_timeline(const _Project *ptr, uintptr_t id);

uintptr_t project_get_timeline_count(const _Project *ptr);

_WrapperErrorCode project_guard_get_project_from_guard(const void *guard_ptr, const _Project **out);

uint64_t clip_get_id(const _Clip *ptr);

int64_t clip_get_position(const _Clip *ptr);

int64_t clip_get_duration(const _Clip *ptr);

void project_debug_log(const _Project *ptr);

_WrapperErrorCode layer_find_clip_at_frame(const _Layer *ptr, int64_t frame, const _Clip **out);

uintptr_t layer_get_clips_count(const _Layer *ptr);

_StringView layer_get_name(const _Layer *ptr);

_WrapperErrorCode layer_clips_begin(const _Layer *layer, _ClipIterator **out);

_WrapperErrorCode layer_clips_in_range_begin(const _Layer *layer,
                                             int64_t start,
                                             int64_t end,
                                             _ClipIterator **out);

_WrapperErrorCode clip_iter_next(_ClipIterator *iter_ptr, const _Clip **out);

_WrapperErrorCode clip_iter_free(_ClipIterator *iter);

const _Layer *timeline_get_layer_by_layer_handle(const _Timeline *ptr, uint32_t layer_handle);

const _Layer *timeline_get_layer_by_sorted_idx(const _Timeline *ptr, uint32_t index);

uintptr_t timeline_get_layers_count(const _Timeline *ptr);

_WrapperErrorCode timeline_find_clip_by_id(const _Timeline *ptr,
                                           uint64_t clip_id,
                                           const _Clip **out_clip,
                                           uintptr_t *out_layer_idx);

bool timeline_can_place_clip_at(const _Timeline *ptr,
                                uint32_t layer_idx,
                                int64_t position,
                                int64_t duration,
                                const uint64_t *exclude_ids_ptr,
                                uintptr_t exclude_ids_len);

_StringView wgpuutil_init_surface(void *window_ptr,
                                  void *display_ptr,
                                  uint32_t width,
                                  uint32_t height,
                                  bool is_wayland,
                                  _WGpuUtil **out);

_StringView wgpuutil_drop(_WGpuUtil *ptr);

_StringView wgpuutil_update_surface(_WGpuUtil *ptr,
                                    void *window_ptr,
                                    void *display_ptr,
                                    bool is_wayland);

_StringView wgpuutil_update_size(_WGpuUtil *ptr, uint32_t width, uint32_t height);

_StringView wgpuutil_render_frame(_WGpuUtil *ptr_wgpu,
                                  const _ClientNetworkHandler *ptr_network,
                                  const _Timeline *ptr_timeline,
                                  int64_t current_frame);

void req_test(const _ClientNetworkHandler *ptr_network);

void req_new_project(const _ClientNetworkHandler *ptr_network);

_WrapperErrorCode req_load_stream(const _ClientNetworkHandler *ptr_network, _StringView path);

}  // extern "C"

}  // namespace esotereel_gui_helper
