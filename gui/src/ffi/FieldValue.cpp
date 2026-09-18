#include "FieldValue.h"
#include "esotereel_gui_helper.h"
#include "ffi/StringView.h"

namespace esotereel {

using CFieldValue = esotereel_gui_helper::CFieldValue;

static std::string ownedStringToString(const RawOwnedString &raw) {
    return OwnedString::intoStdString(raw);
}

FieldValue::Variant convertFieldValue(const CFieldValue &value) {
    switch (value.tag) {

    case CFieldValueTag::Bool:
        return value.data.bool_value != 0;

    case CFieldValueTag::Int:
        return value.data.int_value;

    case CFieldValueTag::Float:
        return value.data.float_value;

    case CFieldValueTag::Enum:
        return EnumValue{ownedStringToString(value.data.enum_value)};

    case CFieldValueTag::String:
        return ownedStringToString(value.data.string_value);

    case CFieldValueTag::Path: {
        const auto &path = value.data.path_value;

        PathValue result;
        result.value.reserve(path.len);

        for (std::size_t i = 0; i < path.len; ++i) {
            result.value.emplace_back(ownedStringToString(path.ptr[i]));
        }

        return result;
    }

    case CFieldValueTag::Color: {
        const auto &color = value.data.color_value;

        return RgbaColor{
            static_cast<float>(color.r) / 255.0f,
            static_cast<float>(color.g) / 255.0f,
            static_cast<float>(color.b) / 255.0f,
            static_cast<float>(color.a) / 255.0f,
        };
    }

    case CFieldValueTag::Array: {
        const auto &array = value.data.array_value;

        FieldValue::Array result;
        result.reserve(array.len);

        for (std::size_t i = 0; i < array.len; ++i) {
            result.emplace_back(FieldValue::fromC(array.ptr[i]));
        }

        return result;
    }

    case CFieldValueTag::Map: {
        const auto &map = value.data.map_value;

        FieldValue::Map result;

        for (std::size_t i = 0; i < map.len; ++i) {
            const auto &entry = map.ptr[i];

            result.emplace(ownedStringToString(entry.key), FieldValue::fromC(entry.value));
        }

        return result;
    }
    }

    throw std::runtime_error("Invalid CFieldValueTag");
}

FieldValue FieldValue::fromC(const CFieldValue &value) {
    return FieldValue(convertFieldValue(value));
}

} // namespace esotereel
