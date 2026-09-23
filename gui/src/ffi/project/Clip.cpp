#include "Clip.h"
#include "esotereel_gui_helper.h"

#include "ffi/ClientState.h"
#include "ffi/CommandQueue.h"
#include "ffi/StringView.h"

namespace esotereel {

Clip::Clip(const RawClip *p) noexcept : ptr_clip(p) {}

Clip Clip::Empty() {
    return Clip(nullptr);
}

bool Clip::isValid() const noexcept {
    return ptr_clip != nullptr;
}

ClipId Clip::id() const noexcept {
    return esotereel_gui_helper::clip_get_id(ptr_clip);
}

TimelineTick Clip::position() const noexcept {
    return esotereel_gui_helper::clip_get_position(ptr_clip);
}

TimelineTick Clip::duration() const noexcept {
    return esotereel_gui_helper::clip_get_duration(ptr_clip);
}

// ClipPropertyValue ClipPropertyValue::fromC(const CClipBindingValue &value) {
//     switch (value.tag) {
//     case CClipBindingValueTag::Static:
//         return ClipPropertyValue::fromStatic(FieldValue::fromC(value.data.static_value));
//     }

//     throw std::runtime_error("Invalid CClipBindingValueTag");
// }

// CClipBindingValue ClipPropertyValue::toC() const {
//     CClipBindingValue result{};

//     std::visit(
//         [&](auto &&arg) {
//             using T = std::decay_t<decltype(arg)>;

//             if constexpr (std::is_same_v<T, FieldValue>) {
//                 result.tag = CClipBindingValueTag::Static;
//                 result.data.static_value = arg.toC();
//             }
//         },
//         value);

//     return result;
// }

// void ClipPropertyValue::freeC(CClipBindingValue &value) {
//     switch (value.tag) {
//     case CClipBindingValueTag::Static:
//         FieldValue::freeC(value.data.static_value);
//         break;
//     }
// }

Result<ClipPropertyValue> Clip::getPropertyValue(const QString &key) const {
    QByteArray keyUtf8 = key.toUtf8();
    RawStringView keyView = StringView::fromQUtf8String(keyUtf8);

    auto result = esotereel_gui_helper::clip_get_property_value(ptr_clip, keyView);

    if (!result.is_ok) {
        return Result<ClipPropertyValue>::err(OwnedString::intoStdString(result.value.err));
    }

    if (result.value.ok.has_value) {
        return Result<ClipPropertyValue>::ok(ClipPropertyValue::fromC(result.value.ok.value.some));
    } else {
        return Result<ClipPropertyValue>::err("Property not found");
    }
}

Result<QVector<ClipPropertySchema>> Clip::getAllFields(ClientState *network) const {
    if (!network) {
        return Result<QVector<ClipPropertySchema>>::err("Network handler is null");
    }

    auto result = esotereel_gui_helper::clip_get_all_fields(*network, ptr_clip);

    if (!result.is_ok) {
        return Result<QVector<ClipPropertySchema>>::err(OwnedString::intoStdString(result.value.err));
    }

    auto &array = result.value.ok;

    QVector<ClipPropertySchema> fields;
    fields.reserve(static_cast<int>(array.len));

    for (std::size_t i = 0; i < array.len; ++i) {
        fields.append(ClipPropertySchema(array.ptr[i]));
    }

    array.free_fn(&array);

    return Result<QVector<ClipPropertySchema>>::ok(fields);
}

Result<QStringList> Clip::getCategories(ClientState *network) const {
    if (!network) {
        return Result<QStringList>::err("Network handler is null");
    }

    auto result = esotereel_gui_helper::clip_get_categories(*network, ptr_clip);

    if (!result.is_ok) {
        return Result<QStringList>::err(OwnedString::intoStdString(result.value.err));
    }

    auto &array = result.value.ok;

    QStringList categories;
    categories.reserve(static_cast<int>(array.len));

    for (std::size_t i = 0; i < array.len; ++i) {
        categories.append(OwnedString::toQString(array.ptr[i]));
        OwnedString::free(array.ptr[i]);
    }

    array.free_fn(&array);

    return Result<QStringList>::ok(categories);
}

Result<void> Clip::setPropertyValue(CommandQueue &commandQueue, TimelineId timelineId, const QString &key,
                                    const ClipPropertyValue &value) const {
    QByteArray keyUtf8 = key.toUtf8();
    RawStringView keyView = StringView::fromQUtf8String(keyUtf8);

    CClipBindingValue cValue = value.toC();

    auto result = esotereel_gui_helper::clip_set_property_value(commandQueue, timelineId, this->id(), keyView, cValue);

    ClipPropertyValue::freeC(cValue);

    if (!result.is_ok) {
        return Result<void>::err(OwnedString::intoStdString(result.err));
    }

    return Result<void>::ok();
}

} // namespace esotereel
