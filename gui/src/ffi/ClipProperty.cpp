#include "ClipProperty.h"

#include "FfiConvert.h"
#include "ffi/ClipProperty.h"

namespace esotereel::window {

using RawOwnedString = esotereel_gui_helper::OwnedString;
using FfiPropertySchema = esotereel_gui_helper::FfiPropertySchema;

Result<QVector<ClipPropertySchema>> getAllFields(ClientState *state, const Clip &clip) {
    auto raw = esotereel_gui_helper::clip_get_all_fields(*state, clip);
    return convertArrayResult(raw, [](const FfiPropertySchema &schema) { return ClipPropertySchema(schema); });
}

Result<QStringList> getCategories(ClientState *state, const Clip &clip) {
    auto raw = esotereel_gui_helper::clip_get_categories(*state, clip);
    return convertArrayResult(raw, [](const RawOwnedString &owned) { return esotereel::OwnedString::toQString(owned); });
}

Result<std::optional<ClipPropertyValue>> getValue(const Clip &clip, const QString &key) {
    auto raw = esotereel_gui_helper::clip_get_property_value(clip, esotereel::StringView::fromStdString(key.toStdString()));
    return convertOptionResult(raw, [](const CClipBindingValue &value) { return ClipPropertyValue::fromC(value); });
}

void setValue(CommandQueue &queue, const TimelineId &timelineId, const ClipId &clipId, const QString &key,
              const ClipPropertyValue &value) {
    esotereel_gui_helper::clip_set_property_value(queue, timelineId, clipId,
                                                  esotereel::StringView::fromStdString(key.toStdString()), value.toC());
}

} // namespace esotereel::window
