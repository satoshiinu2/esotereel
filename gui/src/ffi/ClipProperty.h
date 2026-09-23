#pragma once

#include <optional>

#include <QString>
#include <QVector>

#include "ClipPropertyValue.h"
#include "ffi/Settings.h"
#include "ffi/project/Clip.h"

namespace esotereel::window {
using RawOwnedString = esotereel_gui_helper::OwnedString;
using FfiPropertySchema = esotereel_gui_helper::FfiPropertySchema;

// clip_get_all_fields のラッパー。ClipPropertySchemaを使用
Result<QVector<ClipPropertySchema>> getAllFields(ClientState *state, const Clip &clip);

// clip_get_categories のラッパー。
Result<QStringList> getCategories(ClientState *state, const Clip &clip);

// clip_get_property_value のラッパー。未設定(None)ならstd::nulloptを返す。
Result<std::optional<ClipPropertyValue>> getValue(const Clip &clip, const QString &key);

// clip_set_property_value のラッパー。コマンドキューに積むだけなので同期エラーは通常起きない想定。
void setValue(CommandQueue &queue, const TimelineId &timelineId, const ClipId &clipId, const QString &key,
              const ClipPropertyValue &value);

} // namespace esotereel::window
