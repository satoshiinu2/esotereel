#include "FieldValue.h"
#include "esotereel_gui_helper.h"
#include "ffi/StringView.h"

namespace esotereel {

using CFieldValue = esotereel_gui_helper::CFieldValue;

FieldValue::Variant convertFieldValue(const CFieldValue &value) {
    switch (value.tag) {

    case CFieldValueTag::Bool:
        return value.data.bool_value != 0;

    case CFieldValueTag::Int:
        return value.data.int_value;

    case CFieldValueTag::Float:
        return value.data.float_value;

    case CFieldValueTag::Enum:
        return EnumValue{OwnedString::intoStdString(value.data.enum_value)};

    case CFieldValueTag::String:
        return OwnedString::intoStdString(value.data.string_value);

    case CFieldValueTag::Path: {
        const auto &path = value.data.path_value;

        PathValue result;
        result.value.reserve(path.len);

        for (std::size_t i = 0; i < path.len; ++i) {
            result.value.emplace_back(OwnedString::intoStdString(path.ptr[i]));
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

            result.emplace(OwnedString::intoStdString(entry.key), FieldValue::fromC(entry.value));
        }

        return result;
    }
    }

    throw std::runtime_error("Invalid CFieldValueTag");
}

FieldValue FieldValue::fromC(const CFieldValue &value) {
    return FieldValue(convertFieldValue(value));
}

CFieldValue FieldValue::toC() const {
    CFieldValue result{};

    std::visit(
        [&](auto &&arg) {
            using T = std::decay_t<decltype(arg)>;

            if constexpr (std::is_same_v<T, bool>) {
                result.tag = CFieldValueTag::Bool;
                result.data.bool_value = arg ? 1 : 0;

            } else if constexpr (std::is_same_v<T, std::int64_t>) {
                result.tag = CFieldValueTag::Int;
                result.data.int_value = arg;

            } else if constexpr (std::is_same_v<T, double>) {
                result.tag = CFieldValueTag::Float;
                result.data.float_value = arg;

            } else if constexpr (std::is_same_v<T, EnumValue>) {
                result.tag = CFieldValueTag::Enum;
                result.data.enum_value = OwnedString::fromStdString(arg.value);

            } else if constexpr (std::is_same_v<T, std::string>) {
                result.tag = CFieldValueTag::String;
                result.data.string_value = OwnedString::fromStdString(arg);

            } else if constexpr (std::is_same_v<T, PathValue>) {
                result.tag = CFieldValueTag::Path;

                const auto len = arg.value.size();
                auto *entries = len > 0 ? new RawOwnedString[len] : nullptr;

                for (std::size_t i = 0; i < len; ++i) {
                    entries[i] = OwnedString::fromStdString(arg.value[i]);
                }

                result.data.path_value = esotereel_gui_helper::FfiOwnedStringArray{
                    entries,
                    len,
                    len,
                };

            } else if constexpr (std::is_same_v<T, RgbaColor>) {
                result.tag = CFieldValueTag::Color;
                result.data.color_value = esotereel_gui_helper::RgbaColor{
                    arg.r,
                    arg.g,
                    arg.b,
                    arg.a,
                };

            } else if constexpr (std::is_same_v<T, FieldValue::Array>) {
                result.tag = CFieldValueTag::Array;

                const auto len = arg.size();
                auto *entries = len > 0 ? new CFieldValue[len] : nullptr;

                for (std::size_t i = 0; i < len; ++i) {
                    entries[i] = arg[i].toC();
                }

                result.data.array_value = esotereel_gui_helper::CFieldValueArray{
                    entries,
                    len,
                    len,
                };

            } else if constexpr (std::is_same_v<T, FieldValue::Map>) {
                result.tag = CFieldValueTag::Map;

                const auto len = arg.size();
                auto *entries = len > 0 ? new esotereel_gui_helper::CFieldValueMapEntry[len] : nullptr;

                std::size_t i = 0;
                for (const auto &[key, val] : arg) {
                    entries[i].key = OwnedString::fromStdString(key);
                    entries[i].value = val.toC();
                    ++i;
                }

                result.data.map_value = esotereel_gui_helper::CFieldValueMap{
                    entries,
                    len,
                    len,
                };
            }
        },
        value_);

    return result;
}

void FieldValue::freeC(CFieldValue &value) {
    switch (value.tag) {
    case CFieldValueTag::Bool:
    case CFieldValueTag::Int:
    case CFieldValueTag::Float:
    case CFieldValueTag::Color:
        break;

    case CFieldValueTag::Enum:
        OwnedString::free(value.data.enum_value);
        break;

    case CFieldValueTag::String:
        OwnedString::free(value.data.string_value);
        break;

    case CFieldValueTag::Path: {
        auto &path = value.data.path_value;
        for (std::size_t i = 0; i < path.len; ++i) {
            OwnedString::free(path.ptr[i]);
        }
        delete[] path.ptr;
        path.ptr = nullptr;
        path.len = 0;
        path.capacity = 0;
        break;
    }

    case CFieldValueTag::Array: {
        auto &array = value.data.array_value;
        for (std::size_t i = 0; i < array.len; ++i) {
            freeC(array.ptr[i]);
        }
        delete[] array.ptr;
        array.ptr = nullptr;
        array.len = 0;
        array.capacity = 0;
        break;
    }

    case CFieldValueTag::Map: {
        auto &map = value.data.map_value;
        for (std::size_t i = 0; i < map.len; ++i) {
            OwnedString::free(map.ptr[i].key);
            freeC(map.ptr[i].value);
        }
        delete[] map.ptr;
        map.ptr = nullptr;
        map.len = 0;
        map.capacity = 0;
        break;
    }
    }
}

} // namespace esotereel
