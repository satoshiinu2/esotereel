#pragma once

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

namespace muscedit_lib {

struct Clip;

struct Layer;

struct Project;

struct Timeline;

using OnSendFn = void(*)(const uint8_t*, uintptr_t);

struct CommandCallbacks {
  void (*on_test)();
};

struct ResponseCallbacks {
  void (*on_test)();
};

extern "C" {

void set_send_callbacks(OnSendFn callback);

const Project *get_project();

void set_command_callbacks(CommandCallbacks callbacks);

void on_command_recveve(const uint8_t *ptr, uintptr_t len);

void cmd_test();

void cmd_new_project();

const Timeline *project_get_timeline(const Project *ptr, uintptr_t idx);

uint32_t clip_get_id(const Clip *ptr);

int64_t clip_get_position(const Clip *ptr);

int64_t clip_get_duration(const Clip *ptr);

const Clip *layer_get_clip(const Layer *ptr, uintptr_t l_idx);

uintptr_t layer_get_clips_count(const Layer *ptr);

const uint8_t *layer_get_name_ptr(const Layer *ptr);

uintptr_t layer_get_name_len(const Layer *ptr);

const Layer *timeline_get_layer(const Timeline *ptr, uintptr_t l_idx);

uintptr_t timeline_get_layers_count(const Timeline *ptr);

int64_t timeline_get_playhead(const Timeline *ptr);

void set_responce_callbacks(ResponseCallbacks callbacks);

void on_responce_recveve(const uint8_t *ptr, uintptr_t len);

void res_test();

void res_project_all();

}  // extern "C"

}  // namespace muscedit_lib
